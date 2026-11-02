# JNA and the UniFFI bindings are reached through reflection (Structure fields, callbacks,
# Native.register); keep them when an app shrinks with R8.
-dontwarn java.awt.*
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.** { public *; }
-keep class org.curv.asd.core.** { *; }
