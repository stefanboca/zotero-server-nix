import base64
import hashlib
import hmac
import json
import secrets
import threading
import time
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import jwt
from cryptography.hazmat.primitives.asymmetric import rsa


ISSUER = "http://127.0.0.1:19090"
CLIENT_ID = "zotero-test"
CLIENT_SECRET = "oidc-test-secret"

private_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
public_numbers = private_key.public_key().public_numbers()
authorization_codes = {}
authorization_codes_lock = threading.Lock()


def base64url(value):
    return base64.urlsafe_b64encode(value).rstrip(b"=").decode()


def integer_base64url(value):
    return base64url(value.to_bytes((value.bit_length() + 7) // 8, "big"))


class Handler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass

    def send_json(self, status, value):
        body = json.dumps(value, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        request = urllib.parse.urlparse(self.path)
        if request.path == "/.well-known/openid-configuration":
            self.send_json(
                200,
                {
                    "issuer": ISSUER,
                    "authorization_endpoint": ISSUER + "/authorize",
                    "token_endpoint": ISSUER + "/token",
                    "jwks_uri": ISSUER + "/jwks",
                    "response_types_supported": ["code"],
                    "subject_types_supported": ["public"],
                    "id_token_signing_alg_values_supported": ["RS256"],
                    "token_endpoint_auth_methods_supported": ["client_secret_basic"],
                    "code_challenge_methods_supported": ["S256"],
                    "scopes_supported": ["openid", "profile", "email"],
                },
            )
            return
        if request.path == "/jwks":
            self.send_json(
                200,
                {
                    "keys": [
                        {
                            "kty": "RSA",
                            "use": "sig",
                            "alg": "RS256",
                            "kid": "test-key",
                            "n": integer_base64url(public_numbers.n),
                            "e": integer_base64url(public_numbers.e),
                        }
                    ]
                },
            )
            return
        if request.path == "/authorize":
            query = urllib.parse.parse_qs(request.query)
            required = ["client_id", "redirect_uri", "state", "nonce", "code_challenge"]
            if any(not query.get(name) for name in required):
                self.send_error(400)
                return
            if query["client_id"][0] != CLIENT_ID:
                self.send_error(400)
                return
            code = secrets.token_urlsafe(24)
            with authorization_codes_lock:
                authorization_codes[code] = {
                    "nonce": query["nonce"][0],
                    "challenge": query["code_challenge"][0],
                    "redirect_uri": query["redirect_uri"][0],
                }
            separator = "&" if "?" in query["redirect_uri"][0] else "?"
            location = query["redirect_uri"][0] + separator + urllib.parse.urlencode(
                {"code": code, "state": query["state"][0]}
            )
            self.send_response(302)
            self.send_header("Location", location)
            self.end_headers()
            return
        self.send_error(404)

    def do_POST(self):
        if urllib.parse.urlparse(self.path).path != "/token":
            self.send_error(404)
            return
        length = int(self.headers.get("Content-Length", "0"))
        form = urllib.parse.parse_qs(self.rfile.read(length).decode())
        authorization = self.headers.get("Authorization", "")
        if not authorization.startswith("Basic "):
            self.send_json(401, {"error": "invalid_client"})
            return
        try:
            client_id, client_secret = base64.b64decode(
                authorization.removeprefix("Basic ")
            ).decode().split(":", 1)
        except (ValueError, UnicodeDecodeError):
            self.send_json(401, {"error": "invalid_client"})
            return
        if not hmac.compare_digest(client_id, CLIENT_ID) or not hmac.compare_digest(
            client_secret, CLIENT_SECRET
        ):
            self.send_json(401, {"error": "invalid_client"})
            return
        code = form.get("code", [""])[0]
        verifier = form.get("code_verifier", [""])[0]
        with authorization_codes_lock:
            authorization = authorization_codes.get(code)
            if authorization is None:
                self.send_json(400, {"error": "invalid_grant"})
                return
            challenge = base64url(hashlib.sha256(verifier.encode()).digest())
            if not hmac.compare_digest(challenge, authorization["challenge"]):
                self.send_json(400, {"error": "invalid_grant"})
                return
            if form.get("redirect_uri", [""])[0] != authorization["redirect_uri"]:
                self.send_json(400, {"error": "invalid_grant"})
                return
            del authorization_codes[code]
        issued = int(time.time())
        id_token = jwt.encode(
            {
                "iss": ISSUER,
                "sub": "oidc-user",
                "aud": CLIENT_ID,
                "iat": issued,
                "exp": issued + 300,
                "nonce": authorization["nonce"],
                "email": "oidc@example.net",
                "email_verified": True,
                "preferred_username": "oidc-user",
                "name": "OIDC User",
            },
            private_key,
            algorithm="RS256",
            headers={"kid": "test-key"},
        )
        self.send_json(
            200,
            {
                "access_token": secrets.token_urlsafe(24),
                "token_type": "Bearer",
                "expires_in": 300,
                "id_token": id_token,
            },
        )


ThreadingHTTPServer(("127.0.0.1", 19090), Handler).serve_forever()
