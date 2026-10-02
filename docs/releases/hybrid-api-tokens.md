# Hybrid API tokens

Native and JSON clients can sign in at `POST /api/session` with email and password, hold a fifteen minute access bearer, and rotate with `POST /api/session/refresh`. Profile proof is `GET /api/me`. Sign out is `POST /api/session/logout` with the bearer.

The Maud site and Capacitor WebView keep `ecclesia_sid` cookies. Revoke, sign out everywhere, and password change still delete session rows on the primary.

Spec: [0003-hybrid-access-refresh](../specs/0003-hybrid-access-refresh/index.md).
