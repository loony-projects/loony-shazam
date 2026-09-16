# Add project specific ProGuard rules here.
# Keep Gson-serialized DTOs intact (field names are the wire contract).
-keep class com.loonyshazam.app.data.network.dto.** { *; }
-keepattributes Signature
-keepattributes *Annotation*
