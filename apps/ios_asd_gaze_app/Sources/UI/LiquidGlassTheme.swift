import SwiftUI
import UIKit

/// Semantic palette and design tokens for the Liquid Glass aesthetic.
///
/// The canvas + card tokens are *adaptive*: deep twilight surfaces in dark
/// mode, airy light surfaces in light mode, so every screen follows the
/// system theme on launch. Accent hues stay identical in both modes.
/// Text must use `.primary` / `.secondary` (never fixed white) on any
/// surface painted from these tokens. Fixed white is reserved for media
/// contrast treatments directly over video, photo, or stimulus fields.
public enum LiquidGlass {
    public static let celestialTeal = Color(red: 0.0, green: 0.824, blue: 0.769)
    public static let warmAmber = Color(red: 1.0, green: 0.718, blue: 0.302)
    public static let emeraldMint = Color(red: 0.0, green: 0.902, blue: 0.463)
    public static let deactivatedBrown = Color(red: 0.62, green: 0.42, blue: 0.28)
    public static let deepCanvas = adaptive(
        dark: Color(red: 0.04, green: 0.06, blue: 0.10),
        light: Color(red: 0.95, green: 0.96, blue: 0.98)
    )
    public static let deepCanvasSecondary = adaptive(
        dark: Color(red: 0.08, green: 0.11, blue: 0.18),
        light: Color(red: 0.88, green: 0.91, blue: 0.95)
    )
    public static let cardBackground = adaptive(
        dark: Color.white.opacity(0.08),
        light: Color.black.opacity(0.05)
    )
    public static let cardBorder = adaptive(
        dark: Color.white.opacity(0.18),
        light: Color.black.opacity(0.12)
    )

    // MARK: - Central Typography & Text Colors (Single Source of Truth)
    public static let textPrimary = adaptive(
        dark: Color.white.opacity(0.96),
        light: Color(red: 0.08, green: 0.10, blue: 0.14)
    )
    public static let textSecondary = adaptive(
        dark: Color.white.opacity(0.65),
        light: Color(red: 0.38, green: 0.42, blue: 0.48)
    )
    public static let textTertiary = adaptive(
        dark: Color.white.opacity(0.40),
        light: Color(red: 0.55, green: 0.58, blue: 0.63)
    )
    public static let textHeadline = adaptive(
        dark: Color.white.opacity(0.92),
        light: Color(red: 0.06, green: 0.08, blue: 0.12)
    )
    public static let dividerColor = adaptive(
        dark: Color.white.opacity(0.12),
        light: Color.black.opacity(0.08)
    )

    // MARK: - Adaptive Stimulus Panes (Clinical Observation)
    public static let stimulusSocialTop = adaptive(
        dark: Color(red: 0.10, green: 0.16, blue: 0.28),
        light: Color(red: 0.88, green: 0.93, blue: 0.99)
    )
    public static let stimulusSocialBottom = adaptive(
        dark: Color(red: 0.05, green: 0.08, blue: 0.16),
        light: Color(red: 0.80, green: 0.88, blue: 0.97)
    )
    public static let stimulusGeometryTop = adaptive(
        dark: Color(red: 0.20, green: 0.08, blue: 0.24),
        light: Color(red: 0.95, green: 0.90, blue: 0.98)
    )
    public static let stimulusGeometryBottom = adaptive(
        dark: Color(red: 0.08, green: 0.04, blue: 0.12),
        light: Color(red: 0.88, green: 0.81, blue: 0.94)
    )

    public static let defaultCornerRadius: CGFloat = 16.0
}

/// Resolves per-environment so screens follow the system theme on launch.
private func adaptive(dark: Color, light: Color) -> Color {
    Color(UIColor { trait in
        trait.userInterfaceStyle == .dark ? UIColor(dark) : UIColor(light)
    })
}

/// ViewModifier implementing the Liquid Glass card styling.
public struct GlassCardModifier: ViewModifier {
    public var cornerRadius: CGFloat
    public var padding: CGFloat

    public init(cornerRadius: CGFloat = LiquidGlass.defaultCornerRadius, padding: CGFloat = 16.0) {
        self.cornerRadius = cornerRadius
        self.padding = padding
    }

    public func body(content: Content) -> some View {
        content
            .padding(padding)
            .background(
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .fill(.ultraThinMaterial)
            )
            .overlay(
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .stroke(
                        LinearGradient(
                            colors: [LiquidGlass.cardBorder, Color.white.opacity(0.04)],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        ),
                        lineWidth: 1.0
                    )
            )
            .clipShape(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous))
            .shadow(color: Color.black.opacity(0.25), radius: 10, x: 0, y: 5)
    }
}

public extension View {
    func glassCard(cornerRadius: CGFloat = LiquidGlass.defaultCornerRadius, padding: CGFloat = 16.0) -> some View {
        modifier(GlassCardModifier(cornerRadius: cornerRadius, padding: padding))
    }
}

/// Frosted Glass Button styled for caregiver actions.
public struct GlassButton: View {
    public let title: String
    public let systemImage: String?
    public let isPrimary: Bool
    public let action: () -> Void

    public init(
        title: String,
        systemImage: String? = nil,
        isPrimary: Bool = true,
        action: @escaping () -> Void
    ) {
        self.title = title
        self.systemImage = systemImage
        self.isPrimary = isPrimary
        self.action = action
    }

    public var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                if let systemImage {
                    Image(systemName: systemImage)
                        .font(.system(size: 16, weight: .semibold))
                }
                Text(title)
                    .font(.system(size: 16, weight: .semibold, design: .rounded))
            }
            .foregroundColor(isPrimary ? .black : .primary)
            .padding(.horizontal, 20)
            .padding(.vertical, 12)
            .frame(maxWidth: .infinity)
            .background(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(isPrimary ? LiquidGlass.celestialTeal : LiquidGlass.cardBackground)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .stroke(isPrimary ? Color.clear : LiquidGlass.cardBorder, lineWidth: 1)
            )
            .shadow(color: isPrimary ? LiquidGlass.celestialTeal.opacity(0.3) : .clear, radius: 8, y: 4)
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Single Source of Truth Typography Components

/// Standardized typography scale ensuring complete light and dark theme legibility.
public enum GlassTextStyle {
    case largeTitle     // 28pt bold rounded
    case title          // 20pt bold rounded
    case sectionHeader  // 14pt bold rounded
    case headline       // 14pt semibold
    case body           // 13pt regular
    case subheadline    // 12pt medium
    case caption        // 11pt regular
    case metricNumber   // 24pt bold rounded
}

public struct GlassTextModifier: ViewModifier {
    public let style: GlassTextStyle
    public let color: Color?

    public func body(content: Content) -> some View {
        switch style {
        case .largeTitle:
            content
                .font(.system(size: 28, weight: .bold, design: .rounded))
                .foregroundColor(color ?? LiquidGlass.textPrimary)
        case .title:
            content
                .font(.system(size: 20, weight: .bold, design: .rounded))
                .foregroundColor(color ?? LiquidGlass.textPrimary)
        case .sectionHeader:
            content
                .font(.system(size: 14, weight: .bold, design: .rounded))
                .foregroundColor(color ?? LiquidGlass.textHeadline)
        case .headline:
            content
                .font(.system(size: 14, weight: .semibold, design: .default))
                .foregroundColor(color ?? LiquidGlass.textPrimary)
        case .body:
            content
                .font(.system(size: 13, weight: .regular, design: .default))
                .foregroundColor(color ?? LiquidGlass.textPrimary)
        case .subheadline:
            content
                .font(.system(size: 12, weight: .medium, design: .default))
                .foregroundColor(color ?? LiquidGlass.textSecondary)
        case .caption:
            content
                .font(.system(size: 11, weight: .regular, design: .default))
                .foregroundColor(color ?? LiquidGlass.textSecondary)
        case .metricNumber:
            content
                .font(.system(size: 24, weight: .bold, design: .rounded))
                .foregroundColor(color ?? LiquidGlass.textPrimary)
        }
    }
}

public extension View {
    func glassStyle(_ style: GlassTextStyle, color: Color? = nil) -> some View {
        modifier(GlassTextModifier(style: style, color: color))
    }
}

/// Centralized Section Header adhering strictly to Liquid Glass & Apple HIG themes.
/// Automatically adapts icon and typography for flawless contrast in both light and dark mode.
public struct GlassSectionHeader: View {
    public let title: String
    public let icon: String?
    public var iconColor: Color

    public init(title: String, icon: String? = nil, iconColor: Color = LiquidGlass.celestialTeal) {
        self.title = title
        self.icon = icon
        self.iconColor = iconColor
    }

    public var body: some View {
        HStack(spacing: 6) {
            if let icon {
                Image(systemName: icon)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundColor(iconColor)
            }
            Text(title)
                .glassStyle(.sectionHeader)
        }
        .padding(.horizontal, 4)
    }
}

/// Theme-adaptive subtle Divider.
public struct GlassDivider: View {
    public init() {}

    public var body: some View {
        Divider()
            .overlay(LiquidGlass.dividerColor)
    }
}

/// Standardized hardware / sensor diagnostic row adhering to light and dark theme contrast.
public struct GlassDiagnosticRow: View {
    public let title: String
    public let subtitle: String
    public let statusText: String
    public let isGood: Bool

    public init(title: String, subtitle: String, statusText: String, isGood: Bool) {
        self.title = title
        self.subtitle = subtitle
        self.statusText = statusText
        self.isGood = isGood
    }

    public var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .glassStyle(.headline)
                Text(subtitle)
                    .glassStyle(.caption)
            }
            Spacer()
            Text(statusText)
                .font(.system(size: 11, weight: .bold, design: .monospaced))
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(Capsule().fill(isGood ? LiquidGlass.emeraldMint.opacity(0.18) : LiquidGlass.warmAmber.opacity(0.18)))
                .foregroundColor(isGood ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
        }
    }
}

