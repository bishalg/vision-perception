// 5. Mobile packaging — XCFramework / AAR / release lanes.
export const title = 'Mobile builds & release';
export const items = [
  { label: 'Build iOS XCFramework (CurvEngine + OpenCV)', run: './scripts/build_ios_xcframework.sh' },
  { label: 'Build Android AAR (arm64 + armv7)', run: './scripts/build_android_aar.sh' },
  { label: 'Open iOS project in Xcode', run: 'open apps/ios_palm_app/PalmScanner.xcodeproj' },
  { label: 'Open Android app in Android Studio', run: 'open apps/android_palm_app' },
  {
    label: 'Note: TestFlight / App Store lanes',
    note: 'cd apps/ios_palm_app && bundle exec fastlane beta  (see fastlane/Fastfile)',
  },
];
