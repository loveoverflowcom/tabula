"""Synthetic parser/guard regressions; these tests do not prove real OIDC."""

import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import tomllib
import unittest
from unittest import mock
import urllib.parse
import urllib.request
from contextlib import redirect_stdout
from email.message import Message


SPEC = importlib.util.spec_from_file_location("kanidm_acceptance_provider", Path(__file__).with_name("provider.py"))
provider = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(provider)


class ProviderHelperTests(unittest.TestCase):
    def setUp(self):
        self.config = {"provider_origin": provider.ORIGIN, "client_id": provider.CLIENT_ID,
                       "callback_url": provider.CALLBACK, "issuer": provider.ORIGIN + "/oauth2/openid/" + provider.CLIENT_ID,
                       "totp": {"algo": "sha256", "step": 30, "digits": 8,
                                "secret": list(b"12345678901234567890123456789012")}}
        self.query = {"client_id": provider.CLIENT_ID, "redirect_uri": provider.CALLBACK,
                      "response_type": "code", "scope": "openid", "code_challenge_method": "S256",
                      "max_age": "0", "prompt": "login", "state": "synthetic-state", "nonce": "synthetic-nonce",
                      "code_challenge": "synthetic-challenge"}

    def url(self, query=None):
        return provider.ORIGIN + "/ui/oauth2?" + urllib.parse.urlencode(query or self.query)

    def test_fresh_login_binding_preserved(self):
        self.assertEqual(provider.authorization_parameters(self.config, self.url()), self.query)

    def test_missing_or_weakened_login_binding_rejected(self):
        for field, replacement in (("max_age", "15"), ("prompt", "none"), ("scope", "openid email"),
                                   ("code_challenge_method", "plain"), ("state", ""), ("nonce", "")):
            with self.subTest(field=field):
                query = dict(self.query, **{field: replacement})
                with self.assertRaises(provider.HarnessError):
                    provider.authorization_parameters(self.config, self.url(query))

    def test_duplicate_authorization_input_rejected(self):
        with self.assertRaises(provider.HarnessError):
            provider.authorization_parameters(self.config, self.url() + "&state=second")

    def test_provider_origin_escape_rejected(self):
        with self.assertRaises(provider.HarnessError):
            provider.authorization_parameters(self.config, self.url().replace("localhost", "other.invalid", 1))

    def test_actual_provider_callback_can_omit_issuer(self):
        url = provider.CALLBACK + "?code=synthetic-code&state=synthetic-state"
        self.assertEqual(provider.callback_result(self.config, url, self.query), {"callback_url": url})

    def test_supplied_callback_issuer_must_match(self):
        url = provider.CALLBACK + "?code=synthetic-code&state=synthetic-state&iss="
        good = url + urllib.parse.quote(self.config["issuer"], safe="")
        self.assertEqual(provider.callback_result(self.config, good, self.query), {"callback_url": good})
        with self.assertRaises(provider.HarnessError):
            provider.callback_result(self.config, url + "https%3A%2F%2Fother.invalid", self.query)

    def test_callback_errors_targets_and_ambiguity_rejected(self):
        for suffix in ("?error=access_denied&state=synthetic-state", "?code=x&state=other",
                       "?code=x&state=synthetic-state&code=y", "?code=x&state=synthetic-state#code=y"):
            with self.subTest(suffix=suffix):
                with self.assertRaises(provider.HarnessError):
                    provider.callback_result(self.config, provider.CALLBACK + suffix, self.query)
        with self.assertRaises(provider.HarnessError):
            provider.callback_result(self.config, "https://other.invalid/?code=x&state=synthetic-state", self.query)

    def test_consent_form_preserves_html_escaped_token(self):
        parser = provider.Forms()
        parser.feed('<form action="/ui/oauth2/consent" method="post"><input type="hidden" name="consent_token" value="a&amp;b"></form>')
        self.assertEqual(parser.forms, [{"action": "/ui/oauth2/consent", "method": "post", "fields": {"consent_token": "a&b"}}])

    def test_duplicate_provider_form_input_rejected(self):
        parser = provider.Forms()
        with self.assertRaises(provider.HarnessError):
            parser.feed('<form><input name="password"><input name="password"></form>')

    def test_private_config_permissions_required(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "config.json"
            provider.private_json(path, self.config)
            self.assertEqual(provider.read_config(path), self.config)
            os.chmod(path, 0o644)
            with self.assertRaises(provider.HarnessError):
                provider.read_config(path)

    def test_private_config_symlink_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "config.json"
            provider.private_json(path, self.config)
            link = Path(temporary) / "link.json"
            link.symlink_to(path)
            with self.assertRaises(provider.HarnessError):
                provider.read_config(link)

    def synthetic_flow(self, consent=True):
        """A helper-state regression, not provider or HTTPS integration evidence."""
        callback = provider.CALLBACK + "?code=synthetic-code&state=synthetic-state"
        responses = [
            (200, {}, b'<form action="/ui/login/begin" method="post"><input name="username"></form>'),
            (200, {}, b'<form action="/ui/login/totp" method="post"><input name="totp"><input name="password"></form>'),
            (200, {}, b'<form action="/ui/login/pw" method="post"><input name="password"></form>'),
            (303, {"Location": "/ui/oauth2/resume"}, b""),
        ]
        if consent:
            responses.append((200, {}, b'<form action="/ui/oauth2/consent" method="post"><input name="consent_token" value="synthetic-consent"></form>'))
        responses.append((303, {"Location": callback}, b""))
        client = mock.Mock()
        client.request.side_effect = responses
        return client, callback

    def test_full_helper_flow_observes_consent_and_keeps_request(self):
        config = dict(self.config, ca_path="synthetic-ca", username="synthetic-person", password="synthetic-password")
        client, callback = self.synthetic_flow()
        with tempfile.TemporaryDirectory() as temporary:
            receipt = Path(temporary) / "consent.json"
            with mock.patch.object(provider, "Client", return_value=client):
                self.assertEqual(provider.authorize(config, self.url(), receipt), {"callback_url": callback})
            self.assertEqual(json.loads(receipt.read_text()), {"actual_provider_consent_observed": True})
            self.assertEqual(client.request.call_args_list[0].args, ("GET", self.url()))
            self.assertEqual(client.request.call_args_list[3].kwargs["form"], {"password": "synthetic-password"})

    def test_first_flow_cannot_bypass_consent(self):
        config = dict(self.config, ca_path="synthetic-ca", username="synthetic-person", password="synthetic-password")
        client, _ = self.synthetic_flow(consent=False)
        with mock.patch.object(provider, "Client", return_value=client):
            with self.assertRaises(provider.HarnessError):
                provider.authorize(config, self.url())

    def test_repeat_flow_accepts_only_job_local_prior_consent(self):
        config = dict(self.config, ca_path="synthetic-ca", username="synthetic-person", password="synthetic-password")
        client, callback = self.synthetic_flow(consent=False)
        with tempfile.TemporaryDirectory() as temporary:
            receipt = Path(temporary) / "consent.json"
            provider.private_json(receipt, {"actual_provider_consent_observed": True})
            with mock.patch.object(provider, "Client", return_value=client):
                self.assertEqual(provider.authorize(config, self.url(), receipt), {"callback_url": callback})

    def test_rfc6238_published_sha256_vector(self):
        # RFC 6238 Appendix A/B: 32-byte ASCII seed, X=30, t=59 => 46119246.
        # https://www.rfc-editor.org/rfc/rfc6238.html#appendix-B
        self.assertEqual(provider.totp_code(self.config["totp"], 59), "46119246")

    def test_rfc6238_published_sha1_and_sha512_vectors(self):
        sha1 = {"algo": "sha1", "step": 30, "digits": 8, "secret": list(b"12345678901234567890")}
        sha512 = {"algo": "sha512", "step": 30, "digits": 8,
                  "secret": list(b"1234567890123456789012345678901234567890123456789012345678901234")}
        self.assertEqual(provider.totp_code(sha1, 59), "94287082")
        self.assertEqual(provider.totp_code(sha512, 59), "90693936")

    def test_totp_parameters_are_bounded(self):
        for field, value in (("algo", "md5"), ("digits", 9), ("step", 0), ("secret", [256] * 32)):
            with self.subTest(field=field):
                with self.assertRaises(provider.HarnessError):
                    provider.totp_code(dict(self.config["totp"], **{field: value}), 59)

    def test_password_only_flow_cannot_skip_required_mfa(self):
        config = dict(self.config, ca_path="synthetic-ca", username="synthetic-person", password="synthetic-password")
        client, _ = self.synthetic_flow()
        client.request.side_effect = [
            (200, {}, b'<form action="/ui/login/begin" method="post"><input name="username"></form>'),
            (200, {}, b'<form action="/ui/login/pw" method="post"><input name="password"></form>'),
        ]
        with mock.patch.object(provider, "Client", return_value=client):
            with self.assertRaises(provider.HarnessError):
                provider.authorize(config, self.url())

    def test_totp_form_cannot_carry_unexpected_fields(self):
        config = dict(self.config, ca_path="synthetic-ca", username="synthetic-person", password="synthetic-password")
        client, _ = self.synthetic_flow()
        client.request.side_effect = [
            (200, {}, b'<form action="/ui/login/totp" method="post"><input name="totp"><input name="password"><input name="other"></form>'),
        ]
        with mock.patch.object(provider, "Client", return_value=client):
            with self.assertRaises(provider.HarnessError):
                provider.authorize(config, self.url())

    def test_startup_config_uses_supported_v2_defaults_and_explicit_path(self):
        script = Path(__file__).with_name("run.sh").read_text()
        source = script.split('cat > "$private/data/server.toml" <<\'EOF\'\n', 1)[1].split("\nEOF", 1)[0]
        config = tomllib.loads(source)
        self.assertEqual(config["version"], "2")
        self.assertEqual(config["domain"], "localhost")
        self.assertEqual(config["origin"], provider.ORIGIN)
        self.assertNotIn("log_level", config)
        self.assertNotIn("role", config)
        self.assertIn('/sbin/kanidmd -c /data/server.toml configtest', script)
        self.assertIn('/sbin/kanidmd -c /data/server.toml server', script)
        helper = Path(__file__).with_name("provider.py").read_text()
        self.assertIn('"/sbin/kanidmd", "-c", "/data/server.toml", "scripting", "recover-account"', helper)

    def test_ready_health_route_need_not_have_version_header(self):
        client = mock.Mock()
        client.request.side_effect = [(200, {}, b"true"), (200, {"X-KANIDM-VERSION": provider.VERSION}, b"robots")]
        provider.probe_ready(client)
        self.assertEqual([call.args for call in client.request.call_args_list], [("GET", "/status"), ("GET", "/robots.txt")])

    def test_ready_wrong_version_or_unhealthy_status_fails(self):
        for responses, category in (([(200, {}, b"false")], "status_unhealthy"),
                                    ([(200, {}, b"not-json")], "status_invalid"),
                                    ([(200, {}, b"true"), (200, {}, b"robots")], "version_mismatch")):
            with self.subTest(category=category):
                client = mock.Mock()
                client.request.side_effect = responses
                with self.assertRaises(provider.HarnessError) as error:
                    provider.probe_ready(client)
                self.assertEqual(error.exception.category, category)

    def test_startup_log_diagnostics_never_include_raw_output(self):
        raw = "Configuration Parse Failure: invalid enum; password=synthetic-secret\n"
        result = provider.startup_categories(raw)
        self.assertEqual(result, ["configuration_parse"])
        self.assertNotIn("synthetic-secret", json.dumps(result))
        self.assertEqual(provider.startup_categories("token=synthetic-private-token"), ["unclassified_startup_failure"])

    def test_container_diagnostics_exclude_raw_error_and_private_paths(self):
        response = mock.Mock(stdout=json.dumps({"Status": "exited", "ExitCode": 1,
                                                "Error": "synthetic-secret", "PrivatePath": "/private/seed"}))
        with mock.patch.object(provider.subprocess, "run", return_value=response):
            self.assertEqual(provider.container_state("tabula-kanidm-synthetic"), {"state": "exited", "exit_code": 1})

    def test_transport_diagnostics_classify_without_exception_text(self):
        error = provider.urllib.error.URLError(ConnectionRefusedError("synthetic-private-url"))
        self.assertEqual(provider.transport_category(error), "connection_refused")
        self.assertEqual(provider.transport_category(TimeoutError("synthetic-secret")), "transport_timeout")
        self.assertEqual(provider.transport_category(provider.ssl.SSLCertVerificationError("synthetic-cert")), "tls_certificate_verification")

    def test_exited_provider_publishes_only_sanitized_failure(self):
        client = mock.Mock()
        client.request.side_effect = provider.HarnessError("provider HTTPS transport failed", "connection_refused")
        with tempfile.TemporaryDirectory() as temporary:
            artifacts = Path(temporary)
            with mock.patch.object(provider, "Client", return_value=client), \
                 mock.patch.object(provider, "container_state", return_value={"state": "exited", "exit_code": 1}), \
                 mock.patch.object(provider, "container_startup_categories", return_value=["configuration_parse"]):
                with self.assertRaises(provider.HarnessError):
                    provider.wait_for_provider("synthetic-ca", "tabula-kanidm-synthetic", artifacts)
            report = json.loads((artifacts / "provider-startup.json").read_text())
            self.assertEqual(report, {"status": "failed", "last_probe": "connection_refused",
                                      "container": {"state": "exited", "exit_code": 1},
                                      "known_startup_categories": ["configuration_parse"]})

    def cookie_jar(self, policy=None, attributes="Secure; HttpOnly; Path=/; Domain=localhost", source=None):
        """Only synthetic headers; never a credential or real provider response."""
        response = mock.Mock()
        headers = Message()
        headers.add_header("Set-Cookie", "synthetic=non-secret; " + attributes)
        response.info.return_value = headers
        jar = provider.http.cookiejar.CookieJar(policy)
        jar.extract_cookies(response, urllib.request.Request(source or provider.ORIGIN + "/ui/oauth2"))
        return jar

    def returned_cookie(self, jar, url):
        request = urllib.request.Request(url)
        jar.add_cookie_header(request)
        return request.get_header("Cookie")

    def test_original_stdlib_dotless_domain_cookie_failure_is_reproduced(self):
        jar = self.cookie_jar()
        self.assertEqual(len(list(jar)), 1)
        self.assertIsNone(self.returned_cookie(jar, provider.ORIGIN + "/ui/login/begin"))

    def test_provider_dotless_cookie_adaptation_keeps_exact_origin(self):
        jar = self.cookie_jar(provider.ProviderCookiePolicy())
        self.assertEqual(self.returned_cookie(jar, provider.ORIGIN + "/ui/login/totp"), "synthetic=non-secret")
        for url in ("http://localhost:8443/ui/login/totp", "https://localhost:9443/ui/login/totp",
                    "https://other.localhost:8443/ui/login/totp", "https://other.invalid:8443/ui/login/totp"):
            with self.subTest(url=url):
                self.assertIsNone(self.returned_cookie(jar, url))

    def test_provider_cookie_adaptation_keeps_path_expiry_and_other_domains(self):
        for attributes in ("Secure; Path=/restricted; Domain=localhost", "Secure; Path=/; Domain=localhost; Max-Age=0",
                           "Secure; Path=/; Domain=other.invalid"):
            with self.subTest(attributes=attributes):
                jar = self.cookie_jar(provider.ProviderCookiePolicy(), attributes)
                self.assertIsNone(self.returned_cookie(jar, provider.ORIGIN + "/ui/login/totp"))

    def test_nonzero_cookie_version_keeps_standard_policy(self):
        policy = provider.ProviderCookiePolicy()
        jar = self.cookie_jar(policy)
        cookie = next(iter(jar))
        cookie.version = 1
        request = urllib.request.Request(provider.ORIGIN + "/ui/login/totp")
        self.assertFalse(provider.http.cookiejar.DefaultCookiePolicy().return_ok(cookie, request))
        self.assertFalse(policy.return_ok(cookie, request))

    def test_adapted_cookies_cannot_be_set_from_another_origin(self):
        for source in ("http://localhost:8443/ui/oauth2", "https://localhost:9443/ui/oauth2",
                       "https://other.localhost:8443/ui/oauth2", "https://other.invalid/ui/oauth2"):
            with self.subTest(source=source):
                jar = self.cookie_jar(provider.ProviderCookiePolicy(), source=source)
                self.assertEqual(len(list(jar)), 0)
                self.assertIsNone(self.returned_cookie(jar, provider.ORIGIN + "/ui/login/totp"))

    def test_fixed_authorize_failure_schema_rejects_arbitrary_text(self):
        self.assertEqual(provider.failure_envelope("totp", "form_contract"), {"error": {"stage": "totp", "category": "form_contract"}})
        for stage, category in (("secret-stage", "form_contract"), ("totp", "password=synthetic-private")):
            with self.subTest(stage=stage, category=category):
                with self.assertRaises(provider.HarnessError):
                    provider.failure_envelope(stage, category)

    def test_failure_artifact_and_stdout_are_exact_allowlisted_ids(self):
        with tempfile.TemporaryDirectory() as temporary:
            capture = io.StringIO()
            with mock.patch.dict(os.environ, {"TABULA_KANIDM_TEST_ARTIFACTS": temporary}), redirect_stdout(capture):
                provider.report_authorize_failure("totp", "form_contract")
            expected = {"error": {"stage": "totp", "category": "form_contract"}}
            self.assertEqual(json.loads(capture.getvalue()), expected)
            self.assertEqual(json.loads((Path(temporary) / "provider-authorize-failure.json").read_text()), expected)

    def test_authorize_preserves_fixed_stage_and_discards_arbitrary_error_text(self):
        config = dict(self.config, ca_path="synthetic-ca", username="synthetic-person", password="synthetic-password")
        client = mock.Mock()
        client.request.side_effect = provider.HarnessError("arbitrary token=synthetic-private", "http_status")
        with mock.patch.object(provider, "Client", return_value=client):
            with self.assertRaises(provider.HarnessError) as error:
                provider.authorize(config, self.url())
        self.assertEqual(provider.failure_envelope(error.exception.stage, error.exception.category),
                         {"error": {"stage": "authorization_request", "category": "http_status"}})
        self.assertNotIn("synthetic-private", str(error.exception))


if __name__ == "__main__":
    unittest.main()
