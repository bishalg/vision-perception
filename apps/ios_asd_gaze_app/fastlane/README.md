fastlane documentation
----

# Installation

Make sure you have the latest version of the Xcode command line tools installed:

```sh
xcode-select --install
```

For _fastlane_ installation instructions, see [Installing _fastlane_](https://docs.fastlane.tools/#installing-fastlane)

# Available Actions

### sync_dist_certs

```sh
[bundle exec] fastlane sync_dist_certs
```



### sync_dev_certs

```sh
[bundle exec] fastlane sync_dev_certs
```



### test

```sh
[bundle exec] fastlane test
```



### generate_release_notes

```sh
[bundle exec] fastlane generate_release_notes
```



### validate_app_local

```sh
[bundle exec] fastlane validate_app_local
```



### create_icons

```sh
[bundle exec] fastlane create_icons
```



### validate_privacy_manifest

```sh
[bundle exec] fastlane validate_privacy_manifest
```



### validate_metadata

```sh
[bundle exec] fastlane validate_metadata
```



### appstore_precheck

```sh
[bundle exec] fastlane appstore_precheck
```



----


## iOS

### ios validate_app_local

```sh
[bundle exec] fastlane ios validate_app_local
```

Run local pre-submission validation on files and structure

### ios validate_metadata

```sh
[bundle exec] fastlane ios validate_metadata
```

Validate App Store metadata string lengths and character constraints (and review_notes.txt, max 4000 chars)

### ios validate_privacy_manifest

```sh
[bundle exec] fastlane ios validate_privacy_manifest
```

Validate iOS PrivacyInfo.xcprivacy manifest

### ios release_check

```sh
[bundle exec] fastlane ios release_check
```

Local pre-submission check: metadata, privacy manifest, store entitlements, Fastfile syntax

### ios appstore_precheck

```sh
[bundle exec] fastlane ios appstore_precheck
```

Run App Store precheck rules on metadata

### ios sync_certs

```sh
[bundle exec] fastlane ios sync_certs
```

Sync App Store distribution certificates and provisioning profiles for App & Extensions

### ios build_app_store

```sh
[bundle exec] fastlane ios build_app_store
```

Build and archive production .ipa

### ios beta

```sh
[bundle exec] fastlane ios beta
```

Build and deploy to TestFlight

### ios update_metadata

```sh
[bundle exec] fastlane ios update_metadata
```

Upload metadata only to App Store Connect (always sends review_notes.txt as App Review notes)

### ios upload_screenshots

```sh
[bundle exec] fastlane ios upload_screenshots
```

Upload screenshots only to App Store Connect

### ios sync_store_listing

```sh
[bundle exec] fastlane ios sync_store_listing
```

Synchronize full store listing (metadata + screenshots)

### ios submit_app_store

```sh
[bundle exec] fastlane ios submit_app_store
```

Submit app for App Store review

----

This README.md is auto-generated and will be re-generated every time [_fastlane_](https://fastlane.tools) is run.

More information about _fastlane_ can be found on [fastlane.tools](https://fastlane.tools).

The documentation of _fastlane_ can be found on [docs.fastlane.tools](https://docs.fastlane.tools).
