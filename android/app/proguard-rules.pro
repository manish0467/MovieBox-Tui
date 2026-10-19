# Keep the JNI bridge class names intact: they are resolved by name at runtime.
-keep class com.moviebox.app.core.NativeBridge { *; }

# kotlinx.serialization
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.**
-keepclassmembers class com.moviebox.app.core.** {
    *** Companion;
}
-keepclasseswithmembers class com.moviebox.app.core.** {
    kotlinx.serialization.KSerializer serializer(...);
}
