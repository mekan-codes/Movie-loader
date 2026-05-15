export function confidenceScore(query: string, title: string): number {
  const normalizedQuery = normalizeMatchKey(query);
  const normalizedTitle = normalizeMatchKey(title);
  if (!normalizedQuery || !normalizedTitle) {
    return 0;
  }
  if (normalizedQuery === normalizedTitle) {
    return 100;
  }
  if (normalizedTitle.includes(normalizedQuery)) {
    return 94;
  }
  if (normalizedQuery.includes(normalizedTitle) && normalizedTitle.length >= 4) {
    return 74;
  }

  const queryTokens = new Set(contentTokens(normalizedQuery));
  const titleTokens = new Set(contentTokens(normalizedTitle));
  if (queryTokens.size === 0 || titleTokens.size === 0) {
    return 0;
  }
  let overlap = 0;
  queryTokens.forEach((token) => {
    if (titleTokens.has(token)) {
      overlap += 1;
    }
  });
  const queryCoverage = overlap / queryTokens.size;
  const titleCoverage = overlap / titleTokens.size;
  return Math.round(Math.max(queryCoverage * 86, titleCoverage * 70));
}

export function stripHtml(value: string): string {
  const element = document.createElement("div");
  element.innerHTML = value;
  return element.textContent?.replace(/\s+/g, " ").trim() ?? "";
}

export function extractYear(value?: string | null): string | null {
  const match = value?.match(/\b(19|20)\d{2}\b/);
  return match?.[0] ?? null;
}

function normalizeMatchKey(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .replace(/\b(season|episode|series|show|movie|film|full|watch|online|hd|uhd|s\d{1,2}|e\d{1,3}|ep\d{1,3})\b/gu, " ")
    .trim()
    .replace(/\s+/g, " ");
}

function contentTokens(value: string): string[] {
  return value
    .split(" ")
    .map((token) => token.trim())
    .filter((token) => token.length > 1)
    .filter(
      (token) =>
        !/^(season|episode|series|show|movie|film|full|watch|online|hd|uhd|s\d{1,2}|e\d{1,3}|ep\d{1,3})$/.test(
          token
        )
    );
}
