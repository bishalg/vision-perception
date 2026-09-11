// 5. Mobile packaging — XCFramework / AAR / release lanes.
export const title = '[MOBILE] Mobile builds & packaging';
export const items = [
  { label: '[BUILD] Build iOS XCFramework (CurvEngine + OpenCV)', run: './scripts/build_ios_xcframework.sh' },
  { label: '[BUILD] Build Android AAR (arm64 + armv7)', run: './scripts/build_android_aar.sh' },
  { label: '[IDE] Open iOS project in Xcode', run: 'open apps/ios_palm_app/PalmScanner.xcodeproj' },
  { label: '[IDE] Open Android app in Android Studio', run: 'open apps/android_palm_app' },
  {
    label: '[NOTE] TestFlight / App Store release lanes',
    note: 'cd apps/ios_palm_app && bundle exec fastlane beta  (see fastlane/Fastfile)',
  },
];
