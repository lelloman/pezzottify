package com.lelloman.simpleaiassistant.model

enum class Language(
    val code: String,
    val displayName: String,
    val nativeName: String,
    val flag: String
) {
    ENGLISH("en", "English", "English", "🇬🇧"),
    SPANISH("es", "Spanish", "Español", "🇪🇸"),
    FRENCH("fr", "French", "Français", "🇫🇷"),
    GERMAN("de", "German", "Deutsch", "🇩🇪"),
    ITALIAN("it", "Italian", "Italiano", "🇮🇹"),
    PORTUGUESE("pt", "Portuguese", "Português", "🇵🇹"),
    DUTCH("nl", "Dutch", "Nederlands", "🇳🇱"),
    POLISH("pl", "Polish", "Polski", "🇵🇱"),
    RUSSIAN("ru", "Russian", "Русский", "🇷🇺"),
    UKRAINIAN("uk", "Ukrainian", "Українська", "🇺🇦"),
    CZECH("cs", "Czech", "Čeština", "🇨🇿"),
    SLOVAK("sk", "Slovak", "Slovenčina", "🇸🇰"),
    HUNGARIAN("hu", "Hungarian", "Magyar", "🇭🇺"),
    ROMANIAN("ro", "Romanian", "Română", "🇷🇴"),
    BULGARIAN("bg", "Bulgarian", "Български", "🇧🇬"),
    GREEK("el", "Greek", "Ελληνικά", "🇬🇷"),
    TURKISH("tr", "Turkish", "Türkçe", "🇹🇷"),
    ARABIC("ar", "Arabic", "العربية", "🇸🇦"),
    HEBREW("he", "Hebrew", "עברית", "🇮🇱"),
    HINDI("hi", "Hindi", "हिन्दी", "🇮🇳"),
    JAPANESE("ja", "Japanese", "日本語", "🇯🇵"),
    KOREAN("ko", "Korean", "한국어", "🇰🇷"),
    CHINESE("zh", "Chinese", "中文", "🇨🇳"),
    THAI("th", "Thai", "ไทย", "🇹🇭"),
    VIETNAMESE("vi", "Vietnamese", "Tiếng Việt", "🇻🇳"),
    INDONESIAN("id", "Indonesian", "Bahasa Indonesia", "🇮🇩"),
    MALAY("ms", "Malay", "Bahasa Melayu", "🇲🇾"),
    SWEDISH("sv", "Swedish", "Svenska", "🇸🇪"),
    NORWEGIAN("no", "Norwegian", "Norsk", "🇳🇴"),
    DANISH("da", "Danish", "Dansk", "🇩🇰"),
    FINNISH("fi", "Finnish", "Suomi", "🇫🇮");

    companion object {
        fun fromCode(code: String): Language? =
            entries.find { it.code == code }
    }
}
