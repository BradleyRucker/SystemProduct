# Auth Setup (Email + Google with Nhost)

This app now supports:
- Email/password sign up + sign in
- Google (Gmail) OAuth sign in

## 1) Create an Nhost project
- In Nhost, create a new project.
- Copy:
  - `Subdomain`
  - `Region`

## 2) Configure local env
- Copy `.env.example` to `.env`.
- Set:
  - `VITE_NHOST_SUBDOMAIN`
  - `VITE_NHOST_REGION`
  - `VITE_NHOST_REDIRECT_TO` (recommended; exact callback URL)

## 3) Enable auth providers in Nhost
- Go to `Authentication -> Sign-in methods` (or equivalent auth provider settings).
- Enable `Email` provider.
- Enable `Google` provider and add your Google OAuth Client ID/Secret.

## 4) Set redirect URLs
- In Nhost auth URL settings:
  - Add your app URL(s) to allowed redirect URLs.
  - For this app's local dev, include `http://localhost:1420/`.
- In Google Cloud OAuth client settings:
  - Authorized JavaScript origin: `http://localhost:1420`
  - Authorized redirect URI: `https://<your-subdomain>.auth.<your-region>.nhost.run/v1/signin/provider/google/callback`
- If your callback URL is strict, set it in `.env` via `VITE_NHOST_REDIRECT_TO` so OAuth uses that exact URL.

## 5) Run app
- Start with `npm run dev`.
- If env vars are present, unauthenticated users see the auth gate first.
- After sign-in, users are routed into the existing app and RBAC UI.

## 6) Role mapping (Nhost user -> app ACL profile)
- The app maps each signed-in user to one ACL profile per project session.
- You can also manage per-project overrides in the app at `/project/:id/access`.
- Role sources (first match wins):
  - Project override (`user_id` or `email`) from Access page
  - `user.metadata.app_role`
  - `user.metadata.access_role`
  - `user.metadata.role`
  - then `user.roles[]`
- Supported values: `admin`, `system`, `subsystem`, `viewer`.
- For subsystem-scoped users, provide one of:
  - `user.metadata.subsystem_id` (preferred: exact node id)
  - `user.metadata.subsystem` / `subsystem_name` / `subsystemName` (matched by subsystem name)

## Release notes
- For production, enforce authorization server-side for all data APIs (frontend RBAC alone is not sufficient).
- Keep email confirmation enabled for public sign-up.
- Prefer one auth identity per person and map it to app roles in a backend table.
