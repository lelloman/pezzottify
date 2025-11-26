# Pezzottify Android Design System

A clean, modern, professional design system for the Pezzottify music streaming app.

## Overview

This design system provides a cohesive visual language using Material 3 (Material You) with custom branding for Pezzottify. The color palette is inspired by music streaming platforms with a vibrant green primary color and complementary earth tones.

## Color Palette

### Primary Color
- **Primary Green (`#1DB954`)**: Vibrant green used for primary actions, highlights, and brand identity
- Inspired by successful music streaming platforms
- Used for buttons, active states, and important UI elements

### Color Roles

The design system uses Material 3's complete color role system:

#### Light Theme
- **Primary**: Vibrant green for main actions and branding
- **Secondary**: Muted teal-green for supporting elements
- **Tertiary**: Blue-grey for accents and variety
- **Background**: Off-white (`#FAFDF A`) for reduced eye strain
- **Surface**: Clean surface colors for cards and containers
- **Error**: Standard red for error states

#### Dark Theme
- **Primary**: Same vibrant green maintaining brand consistency
- **Background**: Deep charcoal (`#191C1A`) for OLED-friendly dark mode
- **Surface**: Elevated surfaces with appropriate contrast
- All color roles properly adapted for dark theme accessibility

### Using Colors in Compose

```kotlin
import androidx.compose.material3.MaterialTheme

@Composable
fun MyComponent() {
    Box(
        modifier = Modifier.background(MaterialTheme.colorScheme.primary)
    ) {
        Text(
            text = "Hello",
            color = MaterialTheme.colorScheme.onPrimary
        )
    }
}
```

### Color Roles Reference

- `primary` / `onPrimary` - Main brand color and text on it
- `primaryContainer` / `onPrimaryContainer` - Tinted containers
- `secondary` / `onSecondary` - Supporting actions
- `tertiary` / `onTertiary` - Accents and variety
- `background` / `onBackground` - Screen background
- `surface` / `onSurface` - Card/component surfaces
- `surfaceVariant` / `onSurfaceVariant` - Subtle differentiation
- `error` / `onError` - Error states
- `outline` / `outlineVariant` - Borders and dividers

## Typography

Complete Material 3 typography scale with optimized sizing and spacing:

### Display Styles
Large, prominent text for hero sections and headlines:
- `displayLarge`: 57sp - Marketing and splash screens
- `displayMedium`: 45sp - Large feature announcements
- `displaySmall`: 36sp - Section dividers

### Headline Styles
Section headers and page titles:
- `headlineLarge`: 32sp / SemiBold - Main page titles
- `headlineMedium`: 28sp / SemiBold - Section headers
- `headlineSmall`: 24sp / SemiBold - Card headers

### Title Styles
Card titles and list item headers:
- `titleLarge`: 22sp / SemiBold - Prominent cards
- `titleMedium`: 16sp / Medium - List items, track names
- `titleSmall`: 14sp / Medium - Supporting titles

### Body Styles
Main content text:
- `bodyLarge`: 16sp - Primary reading text
- `bodyMedium`: 14sp - Standard body text
- `bodySmall`: 12sp - Fine print, metadata

### Label Styles
UI element labels:
- `labelLarge`: 14sp / Medium - Buttons
- `labelMedium`: 12sp / Medium - Tabs, small buttons
- `labelSmall`: 11sp / Medium - Captions, timestamps

### Using Typography

```kotlin
import androidx.compose.material3.MaterialTheme

@Composable
fun TrackItem() {
    Column {
        Text(
            text = "Track Name",
            style = MaterialTheme.typography.titleMedium
        )
        Text(
            text = "Artist Name",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Text(
            text = "3:45",
            style = MaterialTheme.typography.labelSmall
        )
    }
}
```

## Spacing

Consistent spacing scale for layouts:

```kotlin
import com.lelloman.pezzottify.android.ui.theme.Spacing

// Usage
Box(modifier = Modifier.padding(Spacing.Medium)) // 16dp
Row(modifier = Modifier.padding(horizontal = Spacing.Large)) // 24dp
```

### Spacing Scale
- `ExtraSmall`: 4dp - Tight spacing, icon padding
- `Small`: 8dp - Compact lists, chips
- `Medium`: 16dp - Standard spacing (most common)
- `Large`: 24dp - Section spacing
- `ExtraLarge`: 32dp - Large sections
- `ExtraExtraLarge`: 48dp - Page-level spacing

### When to Use
- **4dp**: Internal component padding, icon spacing
- **8dp**: Compact layouts, list item spacing
- **16dp**: Default padding for most components
- **24dp**: Separation between major sections
- **32dp+**: Hero sections, page headers

## Shapes

Modern rounded corners throughout:

```kotlin
import com.lelloman.pezzottify.android.ui.theme.CornerRadius

Card(
    shape = RoundedCornerShape(CornerRadius.Medium)
) { /* ... */ }
```

### Corner Radius Scale
- `ExtraSmall`: 4dp - Chips, small badges
- `Small`: 8dp - Buttons, small cards
- `Medium`: 12dp - Cards, containers
- `Large`: 16dp - Dialogs, large cards
- `ExtraLarge`: 24dp - Bottom sheets, modals

Material 3 shapes are also available via `MaterialTheme.shapes`:
- `shapes.extraSmall`, `shapes.small`, `shapes.medium`, `shapes.large`, `shapes.extraLarge`

## Component Sizes

Predefined sizes for common components:

```kotlin
import com.lelloman.pezzottify.android.ui.theme.ComponentSize

// Album artwork
Image(
    modifier = Modifier.size(ComponentSize.ImageAlbumGrid)
)

// Minimum touch target
IconButton(
    modifier = Modifier.size(ComponentSize.MinTouchTarget)
)
```

### Image Sizes
- `ImageThumbSmall`: 48dp - List thumbnails
- `ImageThumbMedium`: 72dp - Grid thumbnails
- `ImageThumbLarge`: 120dp - Featured items
- `ImageAlbumGrid`: 160dp - Album grid items
- `ImageFullWidth`: 200dp - Detail screens

### UI Elements
- `MinTouchTarget`: 48dp - Accessibility minimum
- `BottomNavHeight`: 56dp - Bottom navigation bar
- `AppBarHeight`: 56dp - Top app bar

## Elevation

Subtle depth using Material 3 elevation:

```kotlin
import com.lelloman.pezzottify.android.ui.theme.Elevation

Card(
    elevation = CardDefaults.cardElevation(
        defaultElevation = Elevation.Medium
    )
)
```

### Elevation Scale
- `Small`: 2dp - Subtle lift
- `Medium`: 4dp - Standard cards
- `Large`: 8dp - Dialogs, modals

## Theme Configuration

### Enabling/Disabling Dynamic Color

By default, dynamic color is **disabled** to maintain consistent branding. Users on Android 12+ can enable it:

```kotlin
PezzottifyTheme(
    dynamicColor = true // Enable Material You dynamic colors
) {
    // App content
}
```

### Forcing Light/Dark Theme

```kotlin
PezzottifyTheme(
    darkTheme = true // Force dark theme
) {
    // App content
}
```

## Design Patterns

### Card Design

```kotlin
Card(
    modifier = Modifier
        .fillMaxWidth()
        .padding(Spacing.Medium),
    shape = RoundedCornerShape(CornerRadius.Medium),
    colors = CardDefaults.cardColors(
        containerColor = MaterialTheme.colorScheme.surfaceVariant
    )
) {
    Column(modifier = Modifier.padding(Spacing.Medium)) {
        Text(
            text = "Card Title",
            style = MaterialTheme.typography.titleMedium
        )
        Spacer(modifier = Modifier.height(Spacing.Small))
        Text(
            text = "Card content",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}
```

### Button Styles

```kotlin
// Primary Action
Button(
    onClick = { /* ... */ },
    modifier = Modifier.padding(Spacing.Medium)
) {
    Text("Play Now")
}

// Secondary Action
OutlinedButton(onClick = { /* ... */ }) {
    Text("Add to Playlist")
}

// Tertiary Action
TextButton(onClick = { /* ... */ }) {
    Text("Cancel")
}
```

### List Item

```kotlin
Row(
    modifier = Modifier
        .fillMaxWidth()
        .clickable { /* ... */ }
        .padding(Spacing.Medium),
    verticalAlignment = Alignment.CenterVertically
) {
    // Thumbnail
    Image(
        modifier = Modifier
            .size(ComponentSize.ImageThumbSmall)
            .clip(RoundedCornerShape(CornerRadius.Small))
    )

    Spacer(modifier = Modifier.width(Spacing.Medium))

    // Content
    Column(modifier = Modifier.weight(1f)) {
        Text(
            text = "Track Name",
            style = MaterialTheme.typography.titleMedium
        )
        Text(
            text = "Artist Name",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }

    // Action
    IconButton(onClick = { /* ... */ }) {
        Icon(Icons.Default.MoreVert, contentDescription = "More")
    }
}
```

## Accessibility

The design system is built with accessibility in mind:

1. **Contrast Ratios**: All color combinations meet WCAG AA standards
2. **Touch Targets**: Minimum 48dp touch targets (`ComponentSize.MinTouchTarget`)
3. **Typography**: Readable font sizes with proper line heights
4. **Dynamic Type**: Supports user font size preferences
5. **Dark Mode**: Full dark theme support for reduced eye strain

## Best Practices

### Do's
✅ Use theme colors via `MaterialTheme.colorScheme`
✅ Use typography styles via `MaterialTheme.typography`
✅ Use spacing values from `Spacing` object
✅ Use shape values from `MaterialTheme.shapes` or `CornerRadius`
✅ Respect minimum touch target sizes
✅ Support both light and dark themes

### Don'ts
❌ Don't hardcode colors (`Color(0xFF...)`)
❌ Don't hardcode text sizes
❌ Don't hardcode spacing values
❌ Don't make touch targets smaller than 48dp
❌ Don't use pure black `#000000` for dark theme backgrounds

## Migration Guide

To update existing components:

### Before
```kotlin
Text(
    text = "Title",
    fontSize = 16.sp,
    fontWeight = FontWeight.Medium,
    color = Color(0xFF1DB954)
)
```

### After
```kotlin
Text(
    text = "Title",
    style = MaterialTheme.typography.titleMedium,
    color = MaterialTheme.colorScheme.primary
)
```

## Resources

- [Material 3 Design Guidelines](https://m3.material.io/)
- [Compose Material 3 Documentation](https://developer.android.com/jetpack/compose/designsystems/material3)
- [Color System](https://m3.material.io/styles/color/the-color-system/color-roles)
- [Typography Scale](https://m3.material.io/styles/typography/type-scale-tokens)

## File Structure

```
ui/src/main/java/com/lelloman/pezzottify/android/ui/theme/
├── Color.kt       # Color definitions
├── Theme.kt       # Theme configuration
├── Type.kt        # Typography system
└── Dimens.kt      # Spacing, sizes, elevation

ui/src/main/res/values/
└── dimens.xml     # XML dimension resources (for compatibility)

app/src/main/res/values/
├── colors.xml     # App-level color resources
└── themes.xml     # App theme configuration
```

## Future Enhancements

Potential additions to the design system:

- [ ] Custom font family (if desired)
- [ ] Animation duration constants
- [ ] Additional semantic color tokens for specific features
- [ ] Component-specific theming extensions
- [ ] Design tokens export for web/iOS consistency
