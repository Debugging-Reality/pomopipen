# Google OAuth client (calendar sync)

Put the OAuth client JSON downloaded from Google Cloud here as `client.json`
(client type **Desktop app**). `build.rs` embeds it, so that build can sign in
to Google Calendar without importing a client in Settings — useful for the
builds shared with friends. The file is git-ignored; never commit it.

Without this file the app still works: Settings → Calendar sync → import the
same JSON there (stored in the app data folder, `google/connection.json`).

Setup steps (Google Cloud Console, once): see docs/GOOGLE_CALENDAR.md.
