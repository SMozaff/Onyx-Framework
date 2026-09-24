# Android app keep rules (Phase 4.5).
#
# The mobile native stack is loaded via System.loadLibrary(core / jni)
# from OnyxApplication and bound through JNI's standard name mangling:
# the C symbols are literally Java_<mangled package+class>_<method>, so the
# Kotlin class names AND the external-fun names must survive R8 verbatim or
# every native binding dies with UnsatisfiedLinkError. The blanket native
# rule below + the explicit class keeps cover all of it.

# Native methods: keep the declaring class' names + the method names so
# JNI mangled symbols keep resolving.
-keepclasseswithmembernames class * {
    native <methods>;
}

# JNI entry-point carriers (each an `object` full of `external fun`s).
-keep class com.onyx.bridge.MobileCoreBridge { *; }
-keep class com.onyx.p2p.P2pCodec { *; }

# WorkManager instantiates the worker by class name (CoroutineWorker
# reflection); keep it and the app/activity shells intact.
-keep class com.onyx.WorkManagerService { *; }
-keep class com.onyx.OnyxApplication { *; }
-keep class com.onyx.MainActivity { *; }

# P2P drivers are referenced reflectively only from their own call sites;
# keeping by name avoids surprises while R8 rules stabilize.
-keep class com.onyx.p2p.WifiDirectDriver { *; }
-keep class com.onyx.p2p.BleDriver { *; }