import QRCode from "qrcode";

const QUIET_ZONE = 4;

/**
 * QR code as an inline SVG drawn from the module matrix (no innerHTML). The `qrcode` library does the
 * encoding; this only draws dark modules as 1x1 squares in one path.
 */
export function QrCode({ value, label, size = 192 }: { value: string; label: string; size?: number }) {
  const { modules } = QRCode.create(value, { errorCorrectionLevel: "M" });
  const n = modules.size;
  let d = "";
  for (let y = 0; y < n; y++) {
    for (let x = 0; x < n; x++) {
      if (modules.data[y * n + x]) d += `M${x + QUIET_ZONE} ${y + QUIET_ZONE}h1v1h-1z`;
    }
  }
  const box = n + QUIET_ZONE * 2;
  return (
    <svg
      role="img"
      aria-label={label}
      width={size}
      height={size}
      viewBox={`0 0 ${box} ${box}`}
      shapeRendering="crispEdges"
      className="qr"
    >
      {/* Always dark-on-white, also in dark mode, so phone cameras can read it. */}
      <rect width={box} height={box} fill="#ffffff" />
      <path d={d} fill="#000000" />
    </svg>
  );
}
