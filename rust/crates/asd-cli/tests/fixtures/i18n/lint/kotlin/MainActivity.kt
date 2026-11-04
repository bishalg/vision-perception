package org.curv.fixture

// Fixture for `asd-cli i18n lint` (WP-L.0, I18N-8). Exactly one finding: `Text("Start")`.
@Composable
fun Home(count: Int) {
    val quote = '"'
    Text("Start")
    Text(stringResource(R.string.home_title))
    Text("$count")
    Text("Curv") // i18n-ok: brand name
}
