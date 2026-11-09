# Signing, secrets and identity (ADR-0005, CI-4)

Kept outside `README.md`, which `fastlane` regenerates on every run.


No Apple ID, team id, review contact or demo credential is committed. Set these in
`fastlane/.env` (copy from `.env.example`) or in CI secrets:
`APP_IDENTIFIER`, `APPLE_TEAM_ID`, `APPLE_ID`, `REVIEW_CONTACT_FIRST_NAME`,
`REVIEW_CONTACT_LAST_NAME`, `REVIEW_CONTACT_EMAIL`, `REVIEW_CONTACT_PHONE`,
`REVIEW_DEMO_USER`, `REVIEW_DEMO_PASSWORD`. `review_notes.txt` (App Review notes) is not secret and is always sent. The `update_metadata` lane passes
`app_review_information:` from these. `fastlane/metadata/review_information/` is git-ignored.

## History cleanup (decision H1, NOT yet run)

Secrets were removed going forward only; earlier commits still contain them. To purge
history later (coordinate with all collaborators, then force-push and rotate anything exposed):

```sh
git filter-repo --invert-paths \
  --path apps/ios_asd_gaze_app/fastlane/metadata/review_information/ \
  --replace-text <(printf '%s\n' 'LITERAL_APPLE_ID==>REMOVED' 'LITERAL_TEAM_ID==>REMOVED')
```

Fill the `--replace-text` file with the literals from the old `Appfile`/`config.yml`
(find them with `git log -p -- apps/ios_asd_gaze_app/fastlane/Appfile`).
