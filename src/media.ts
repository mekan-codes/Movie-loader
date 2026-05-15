const playableVideoExtensions = [
  ".mp4",
  ".m4v",
  ".webm",
  ".ogv",
  ".ogg",
  ".mov",
  ".m3u8",
  ".mpd"
];

export function isPlayableVideoUrl(value?: string | null): boolean {
  if (!value) {
    return false;
  }
  try {
    const url = new URL(value);
    const path = url.pathname.toLowerCase();
    return playableVideoExtensions.some((extension) => path.endsWith(extension));
  } catch {
    const cleanValue = value.split("?")[0]?.split("#")[0]?.toLowerCase() ?? "";
    return playableVideoExtensions.some((extension) => cleanValue.endsWith(extension));
  }
}

export function playableUrlFor(url?: string | null, fallback?: string | null): string | null {
  if (isPlayableVideoUrl(url)) {
    return url ?? null;
  }
  if (isPlayableVideoUrl(fallback)) {
    return fallback ?? null;
  }
  return null;
}
