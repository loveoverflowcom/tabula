import importlib.util
import os
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests/online-match"))
sys.path.insert(0, str(ROOT / "tests/accounts"))

def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value

edge = module("local_dev_edge", Path(__file__).with_name("tls_frontend.py"))
runner = module("local_dev_runner", Path(__file__).with_name("run.py"))

class LocalDevHelpers(unittest.TestCase):
    def test_same_origin_context_and_callback_stay_with_preauth_owner(self):
        for path in ("/api/v1/auth/context", "/api/v1/auth/login",
                     "/api/v1/auth/oidc/callback?code=private&state=private",
                     "/api/v2/auth/enrollment/start", "/api/v2/auth/register"):
            handler = object.__new__(edge.ServiceHandler)
            handler.path = path
            self.assertEqual(handler._upstream_port(), 3001)
        for path in ("/api/v1/matches", "/api/v2/profiles/me", "/api/v2/social/search?q=peer"):
            handler = object.__new__(edge.ServiceHandler)
            handler.path = path
            self.assertEqual(handler._upstream_port(), 3002)

    def test_disposable_setup_cannot_inherit_operator_database_or_key(self):
        with patch.dict(os.environ, {"TABULA_AUTH_DATABASE_URL": "must-not-use",
                                     "TABULA_SERVER_CSRF_KEY": "must-not-use",
                                     "TABULA_KANIDM_DISPOSABLE": "1"}):
            environment = runner.service_environment()
        self.assertNotIn("TABULA_AUTH_DATABASE_URL", environment)
        self.assertNotIn("TABULA_SERVER_CSRF_KEY", environment)
        self.assertEqual(environment["TABULA_KANIDM_DISPOSABLE"], "1")

if __name__ == "__main__":
    unittest.main()
