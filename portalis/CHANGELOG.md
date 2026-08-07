# Changelog

## [Unreleased]

### Backend

- Automatically request and retry BitTorrent fetches when manifest sync adds new media entries.
- Preserve direct BitTorrent peer hints learned during collection synchronization.
- Add an opt-in two-process integration test covering encoded invites, manifest sync, automatic fetching, hashes, and file contents.
