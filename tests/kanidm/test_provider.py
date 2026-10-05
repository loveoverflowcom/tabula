"""Synthetic parser/guard regressions; these tests do not prove real OIDC."""

import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock
import urllib.parse


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


if __name__ == "__main__":
    unittest.main()
