export function cn(...classes: Array<string | undefined | null | false>): string {
  return classes.filter(Boolean).join(' ');
}

export interface ParsedDependencyLink {
  name: string;
  url: string | null;
}

export interface ParsedAddonDescription {
  html: string;
  requiredLibraries: ParsedDependencyLink[];
  optionalLibraries: ParsedDependencyLink[];
}

export interface ParsedChangelogSection {
  title: string;
  html: string;
}

function getBbcodeSizeClass(rawSize: string): string {
  const value = rawSize.replace(/['"]/g, '').trim().toLowerCase();
  const numeric = Number.parseInt(value, 10);

  if (value.startsWith('+')) return 'bbcode-size-lg';
  if (value.startsWith('-')) return 'bbcode-size-sm';
  if (!Number.isFinite(numeric)) return 'bbcode-size-md';
  if (numeric <= 2) return 'bbcode-size-sm';
  if (numeric <= 4) return 'bbcode-size-md';
  if (numeric <= 5) return 'bbcode-size-lg';
  return 'bbcode-size-xl';
}

function htmlNodeToBbcodeish(node: Node): string {
  if (node.nodeType === Node.TEXT_NODE) {
    return node.textContent ?? '';
  }

  if (!(node instanceof HTMLElement)) {
    return '';
  }

  const tag = node.tagName.toLowerCase();
  const children = Array.from(node.childNodes)
    .map((child) => htmlNodeToBbcodeish(child))
    .join('');
  const align = (node.getAttribute('align') || node.style.textAlign || '').trim().toLowerCase();

  const withAlignment = (content: string) => {
    if (align === 'center') return `[center]${content}[/center]`;
    if (align === 'right') return `[right]${content}[/right]`;
    if (align === 'left') return `[left]${content}[/left]`;
    return content;
  };

  switch (tag) {
    case 'br':
      return '\n';
    case 'strong':
    case 'b':
      return `[b]${children}[/b]`;
    case 'em':
    case 'i':
      return `[i]${children}[/i]`;
    case 'u':
      return `[u]${children}[/u]`;
    case 's':
    case 'strike':
    case 'del':
      return `[s]${children}[/s]`;
    case 'center':
      return `[center]${children}[/center]\n\n`;
    case 'blockquote':
      return `[quote]${children}[/quote]\n\n`;
    case 'ul':
    case 'ol':
      return `[list]\n${children}[/list]\n\n`;
    case 'li':
      return `[*]${children.trim()}\n`;
    case 'a': {
      const href = node.getAttribute('href');
      if (!href) return children;
      const text = children.trim() || href;
      return `[url=${href}]${text}[/url]`;
    }
    case 'font': {
      let result = children;
      const size = node.getAttribute('size') || node.style.fontSize;
      if (size) {
        result = `[size=${size}]${result}[/size]`;
      }
      return result;
    }
    case 'span': {
      let result = children;
      const size = node.style.fontSize;
      if (size) {
        result = `[size=${size}]${result}[/size]`;
      }
      return result;
    }
    case 'h1':
    case 'h2':
      return `[size=6][b]${children}[/b][/size]\n\n`;
    case 'h3':
    case 'h4':
      return `[size=5][b]${children}[/b][/size]\n\n`;
    case 'h5':
    case 'h6':
      return `[size=4][b]${children}[/b][/size]\n\n`;
    case 'hr':
      return '\n-----\n';
    case 'img':
      return '';
    case 'p':
    case 'div':
    case 'section':
    case 'article':
      return `${withAlignment(children)}\n\n`;
    default:
      return withAlignment(children);
  }
}

function normalizeRichTextSource(input: string): string {
  let source = input.replace(/\r\n/g, '\n').replace(/\r/g, '\n');

  if (typeof DOMParser !== 'undefined' && /<\/?[a-z][^>]*>/i.test(source)) {
    const doc = new DOMParser().parseFromString(source, 'text/html');
    source = Array.from(doc.body.childNodes)
      .map((node) => htmlNodeToBbcodeish(node))
      .join('');
  }

  source = source.replace(/&nbsp;/gi, ' ');
  return source;
}

function stripBbcodeTags(input: string): string {
  return input
    .replace(/\|c[0-9a-fA-F]{6}([\s\S]*?)\|r/g, '$1')
    .replace(/\[url=[^\]]+\]([\s\S]*?)\[\/url\]/gi, '$1')
    .replace(/\[url\]([\s\S]*?)\[\/url\]/gi, '$1')
    .replace(/\[\/?\w[^\]]*\]/g, '')
    .trim();
}

function parseDependencyItem(rawItem: string): ParsedDependencyLink | null {
  const urlMatch = rawItem.match(/\[url=([^\]]+)\]([\s\S]*?)\[\/url\]/i);
  const name = stripBbcodeTags(rawItem).replace(/^[-:*\s]+/, '').trim();
  if (!name) return null;
  return {
    name,
    url: urlMatch?.[1]?.trim() || null
  };
}

function normalizeDependencyName(raw: string): string {
  return stripBbcodeTags(raw)
    .replace(/^[-:*\s]+/, '')
    .replace(/^[0-9]+[.)-]\s*/, '')
    .replace(/[.,;:]+$/, '')
    .trim();
}

function isLikelyDependencyName(value: string): boolean {
  const name = normalizeDependencyName(value);
  if (!name) return false;
  if (name.length > 80) return false;
  if (/\s{3,}/.test(name)) return false;
  if (/^(required libraries?|dependencies|optional libraries?|optional dependencies?)$/i.test(name)) return false;
  if (/^(please install|otherwise|if you need|the following)/i.test(name)) return false;

  const tokens = name.split(/\s+/).filter(Boolean);
  if (tokens.length > 6) return false;

  return tokens.every((token) => /^(?:Lib[A-Za-z0-9_.-]+|[A-Z][A-Za-z0-9_.-]+(?:UI|Companion|Pins|Menu|Logger|Filters|Vars|Pad)?|[A-Za-z]+(?:-[0-9.]+)?|[0-9.]+)$/.test(token));
}

function splitDependencySentence(input: string): string[] {
  return input
    .split(/,|\band\b|\bor\b/gi)
    .map((part) => normalizeDependencyName(part))
    .filter((part) => isLikelyDependencyName(part));
}

function extractSentenceDependencies(source: string, target: ParsedDependencyLink[], pattern: RegExp): string {
  return source.replace(pattern, (match, libs) => {
    const parsed = splitDependencySentence(libs).map((name) => ({ name, url: null }));
    if (parsed.length === 0) return match;
    target.push(...parsed);
    return '\n';
  });
}

function extractPlainDependencySections(source: string, requiredLibraries: ParsedDependencyLink[], optionalLibraries: ParsedDependencyLink[]): string {
  const lines = source.split('\n');
  const kept: string[] = [];

  let currentTarget: ParsedDependencyLink[] | null = null;
  let currentHeadingPattern: RegExp | null = null;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const plain = normalizeDependencyName(line);
    const lower = plain.toLowerCase();

    if (/^(required libraries?|dependencies|required dependencies)$/i.test(plain)) {
      currentTarget = requiredLibraries;
      currentHeadingPattern = /^(required libraries?|dependencies|required dependencies)$/i;
      continue;
    }

    if (/^(optional libraries?|optional dependencies?|3rd party optional plugins.*)$/i.test(plain)) {
      currentTarget = optionalLibraries;
      currentHeadingPattern = /^(optional libraries?|optional dependencies?|3rd party optional plugins.*)$/i;
      continue;
    }

    if (currentTarget) {
      if (!plain) {
        kept.push(line);
        currentTarget = null;
        currentHeadingPattern = null;
        continue;
      }

      if (/^(please install|please install and activate|if you need|the following)/i.test(lower)) {
        continue;
      }

      if (currentHeadingPattern?.test(plain)) {
        continue;
      }

      if (isLikelyDependencyName(plain)) {
        currentTarget.push({ name: plain, url: null });
        continue;
      }

      currentTarget = null;
      currentHeadingPattern = null;
    }

    kept.push(line);
  }

  return kept.join('\n');
}

function dedupeDependencyLinks(items: ParsedDependencyLink[]): ParsedDependencyLink[] {
  const seen = new Set<string>();
  return items.filter((item) => {
    const key = item.name.toLowerCase();
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function parseAddonDescription(input: string): ParsedAddonDescription {
  const source = normalizeRichTextSource(input);
  let cleaned = source;
  const requiredLibraries: ParsedDependencyLink[] = [];
  const optionalLibraries: ParsedDependencyLink[] = [];

  cleaned = extractSentenceDependencies(
    cleaned,
    requiredLibraries,
    /you must install both\s+([^.!?\n]+?)(?:\.\s|!\s|\?\s|otherwise|$)/gi
  );
  cleaned = extractSentenceDependencies(
    cleaned,
    requiredLibraries,
    /this addon requires(?: the use of)?(?: the following libraries?)?[:\s]+([^.!?\n]+?)(?:\.\s|!\s|\?\s|$)/gi
  );

  cleaned = cleaned.replace(/(?:^|\n)([^\n]*(?:required libraries?|dependencies|requires[^\n]*libraries|libraries separately)[^\n]*\n+)?\[list(?:=[^\]]+)?\]([\s\S]*?)\[\/list\]/gi, (match, heading = '', items) => {
    const context = stripBbcodeTags(`${heading} ${items}`).toLowerCase();
    const parsed = Array.from(items.matchAll(/\[\*\]([\s\S]*?)(?=\[\*\]|$)/gi))
      .map((entry) => parseDependencyItem(entry[1]))
      .filter((entry): entry is ParsedDependencyLink => entry !== null);

    if (parsed.length === 0) return match;

    if (context.includes('optional')) {
      optionalLibraries.push(...parsed);
      return '\n';
    }

    if (
      context.includes('required') ||
      context.includes('dependenc') ||
      context.includes('requires') ||
      context.includes('libraries separately')
    ) {
      requiredLibraries.push(...parsed);
      return '\n';
    }

    return match;
  });

  cleaned = extractPlainDependencySections(cleaned, requiredLibraries, optionalLibraries);

  if (requiredLibraries.length > 0) {
    cleaned = cleaned.replace(/(?:^|\n)[^\n]*(?:required libraries?|dependencies|requires[^\n]*libraries|libraries separately)[^\n]*(?=\n|$)/gi, '\n');
  }
  if (optionalLibraries.length > 0) {
    cleaned = cleaned.replace(/(?:^|\n)[^\n]*(?:optional libraries?|optional dependencies?|the following library is optional)[^\n]*(?=\n|$)/gi, '\n');
  }

  cleaned = cleaned.replace(/\n{3,}/g, '\n\n').trim();

  return {
    html: bbcodeToHtml(cleaned),
    requiredLibraries: dedupeDependencyLinks(requiredLibraries),
    optionalLibraries: dedupeDependencyLinks(optionalLibraries)
  };
}

function isChangelogHeader(line: string): boolean {
  const plain = stripBbcodeTags(line).replace(/:+$/, '').trim();
  if (!plain) return false;
  if (/^#{1,6}\s+/.test(line.trim())) return true;
  if (/^(?:version\s*)?v?\d+(?:\.\d+)+(?:[a-z])?$/i.test(plain)) return true;
  if (/^(?:version\s*)?\d+(?:\.\d+)+(?:[a-z])?(?:,\s*v?\d+(?:\.\d+)+(?:[a-z])?)+$/i.test(plain)) return true;
  return false;
}

function normalizeChangelogTitle(line: string): string {
  return stripBbcodeTags(line).replace(/^#+\s*/, '').replace(/:+$/, '').trim();
}

export function parseAddonChangelog(input: string): ParsedChangelogSection[] {
  const source = normalizeRichTextSource(input).trim();
  if (!source) return [];

  const lines = source.split('\n');
  const sections: ParsedChangelogSection[] = [];
  let currentTitle = '';
  let buffer: string[] = [];

  const flush = () => {
    const body = buffer.join('\n').trim();
    if (!currentTitle && !body) return;
    sections.push({
      title: currentTitle || 'Notes',
      html: bbcodeToHtml(body || currentTitle)
    });
  };

  for (const line of lines) {
    if (isChangelogHeader(line)) {
      if (currentTitle || buffer.length > 0) {
        flush();
        buffer = [];
      }
      currentTitle = normalizeChangelogTitle(line);
      continue;
    }
    buffer.push(line);
  }

  flush();
  return sections.filter((section) => section.title || section.html);
}

/** Parse ESOUI BBCode into sanitized HTML. */
export function bbcodeToHtml(input: string): string {
  if (!input) return '';

  let s = normalizeRichTextSource(input)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');

  s = s.replace(/\|c[0-9a-fA-F]{6}([\s\S]*?)\|r/g, '$1');
  s = s.replace(/\[color=[^\]]+\]([\s\S]*?)\[\/color\]/gi, '$1');
  s = s.replace(/\[font=[^\]]+\]([\s\S]*?)\[\/font\]/gi, '$1');

  s = s.replace(/\[b\]([\s\S]*?)\[\/b\]/gi, '<strong>$1</strong>');
  s = s.replace(/\[i\]([\s\S]*?)\[\/i\]/gi, '<em>$1</em>');
  s = s.replace(/\[u\]([\s\S]*?)\[\/u\]/gi, '<u>$1</u>');
  s = s.replace(/\[s\]([\s\S]*?)\[\/s\]/gi, '<s>$1</s>');

  s = s.replace(/\[size=([^\]]+)\]([\s\S]*?)\[\/size\]/gi, (_m, n, inner) => {
    return `<span class="${getBbcodeSizeClass(n)}">${inner}</span>`;
  });

  s = s.replace(/\[center\]([\s\S]*?)\[\/center\]/gi, '<div class="bbcode-align-center">$1</div>');
  s = s.replace(/\[left\]([\s\S]*?)\[\/left\]/gi, '<div class="bbcode-align-left">$1</div>');
  s = s.replace(/\[right\]([\s\S]*?)\[\/right\]/gi, '<div class="bbcode-align-right">$1</div>');
  s = s.replace(/\[indent\]([\s\S]*?)\[\/indent\]/gi, '<div class="bbcode-indent">$1</div>');
  s = s.replace(/\[quote\]([\s\S]*?)\[\/quote\]/gi, '<blockquote class="bbcode-quote">$1</blockquote>');

  s = s.replace(/\[url=([^\]]+)\]([\s\S]*?)\[\/url\]/gi, (_m, href, text) => {
    const safeHref = href.startsWith('http') ? href : '#';
    return `<a href="${safeHref}" target="_blank" rel="noopener noreferrer" class="bbcode-link">${text}</a>`;
  });
  s = s.replace(/\[url\](https?:\/\/[^[]+?)\[\/url\]/gi, (_m, href) => {
    return `<a href="${href}" target="_blank" rel="noopener noreferrer" class="bbcode-link">${href}</a>`;
  });

  s = s.replace(/\[img\][\s\S]*?\[\/img\]/gi, '');
  s = s.replace(/\[img=[^\]]*\][\s\S]*?\[\/img\]/gi, '');

  s = s.replace(/\[list(?:=[^\]]+)?\]([\s\S]*?)\[\/list\]/gi, (_m, inner) => {
    const items = inner.replace(/\[\*\]([\s\S]*?)(?=\[\*\]|\[\/list\]|$)/gi, '<li>$1</li>');
    return `<ul class="bbcode-list">${items}</ul>`;
  });

  s = s.replace(/\[\*\]/gi, '');

  s = s.replace(/^#{2,3}\s+(.+)$/gm, (_m, text) => `<div class="bbcode-heading">${text}</div>`);
  s = s.replace(/^v?\d+(?:\.\d+)+(?:[a-z])?\s*:?$/gim, (text) => `<div class="bbcode-version">${text.trim()}</div>`);
  s = s.replace(/^(?:-{3,}|_{3,}|={3,})$/gm, '<hr class="bbcode-rule">');
  s = s.replace(/^&gt;\s+(.+)$/gm, '<blockquote class="bbcode-quote">$1</blockquote>');

  s = s.replace(/\[\/?\w[^\]]*\]/g, '');
  s = s.replace(/ style=&quot;[^&]*&quot;/gi, '');

  s = s.replace(/\n{2,}/g, '</p><p class="bbcode-para">');
  s = s.replace(/\n/g, '<br>');
  s = `<p class="bbcode-para">${s}</p>`;
  s = s.replace(/<p class="bbcode-para">\s*(<(?:div|blockquote)[\s\S]*?<\/(?:div|blockquote)>|<hr class="bbcode-rule">)\s*<\/p>/gi, '$1');
  s = s.replace(/<p class="bbcode-para">\s*(<div class="bbcode-(?:heading|version)">[\s\S]*?<\/div>)\s*<\/p>/gi, '$1');

  return s;
}

const bbcodeHtmlCache = new Map<string, string>();
const MAX_BBCODE_CACHE_SIZE = 128;

export function bbcodeToHtmlCached(input: string): string {
  if (!input) return '';
  const cached = bbcodeHtmlCache.get(input);
  if (cached !== undefined) return cached;

  const html = bbcodeToHtml(input);
  if (bbcodeHtmlCache.size >= MAX_BBCODE_CACHE_SIZE) {
    bbcodeHtmlCache.clear();
  }
  bbcodeHtmlCache.set(input, html);
  return html;
}

export function formatBytes(bytes: number, decimals = 2): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(decimals))} ${sizes[i]}`;
}

export function formatDate(date: Date | string): string {
  const d = typeof date === 'string' ? new Date(date) : date;
  return d.toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric'
  });
}

export function formatCompact(n: number): string {
  if (n >= 1_000_000) {
    const v = n / 1_000_000;
    return v >= 10 ? `${Math.round(v)}M` : `${+v.toFixed(1)}M`;
  }
  if (n >= 1_000) {
    const v = n / 1_000;
    return v >= 10 ? `${Math.round(v)}K` : `${+v.toFixed(1)}K`;
  }
  return String(n);
}

const ESOUI_CATEGORY_ORDER = [
  'Action Bar Mods',
  'Auction House & Vendors',
  'Bags, Bank, Inventory',
  'Buff, Debuff, Spell',
  'Casting Bars, Cooldowns',
  'Character Advancement',
  'Chat Mods',
  'Class & Role Specific',
  'Combat Mods',
  'Data Mods',
  'Game Controller',
  'Graphic UI Mods',
  'Group, Guild & Friends',
  'Homestead',
  'Info, Plug-in Bars',
  'Map, Coords, Compass',
  'Mail',
  'PvP',
  'Raid Mods',
  'RolePlay',
  'Tradeskill Mods',
  'ToolTip',
  'UI Media',
  'Unit Mods',
  'Miscellaneous',
  'Utility Mods',
  'Libraries',
  'Developer Utilities',
  'ESO Tools & Utilities',
  'Unofficial game translations',
  'Beta-version AddOns',
  'Plug-Ins & Patches',
  'Discontinued & Outdated'
];

const esouiCategoryOrderMap = new Map(
  ESOUI_CATEGORY_ORDER.map((name, index) => [normalizeCategoryName(name), index])
);

export function normalizeCategoryName(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, ' ')
    .trim();
}

export function compareEsoUiCategoryOrder(a: { name: string }, b: { name: string }): number {
  const aIndex =
    esouiCategoryOrderMap.get(normalizeCategoryName(a.name)) ?? Number.MAX_SAFE_INTEGER;
  const bIndex =
    esouiCategoryOrderMap.get(normalizeCategoryName(b.name)) ?? Number.MAX_SAFE_INTEGER;
  if (aIndex !== bIndex) return aIndex - bIndex;
  return a.name.localeCompare(b.name);
}

export type CategorySection =
  | 'Stand-Alone Addons'
  | 'Class & Role Specific'
  | 'Utilities'
  | 'Optional';

export const CATEGORY_SECTION_ORDER: CategorySection[] = [
  'Stand-Alone Addons',
  'Class & Role Specific',
  'Utilities',
  'Optional'
];

export function getCategorySection(
  category: { name: string; parentId?: string; parentIds?: string[] },
  categoriesById: Map<string, { name: string }>
): CategorySection {
  const ancestorIds = ([category.parentId, ...(category.parentIds ?? [])] as string[]).filter(
    Boolean
  );
  const names = [
    category.name,
    ...ancestorIds.map((id: string) => categoriesById.get(id)?.name ?? '')
  ]
    .map(normalizeCategoryName)
    .filter(Boolean);

  if (names.includes('class role specific')) return 'Class & Role Specific';
  if (
    names.includes('libraries') ||
    names.includes('developer utilities') ||
    names.includes('eso tools utilities')
  ) {
    return 'Utilities';
  }
  if (
    names.includes('unofficial game translations') ||
    names.includes('beta version addons') ||
    names.includes('plug ins patches') ||
    names.includes('discontinued outdated')
  ) {
    return 'Optional';
  }
  return 'Stand-Alone Addons';
}

export function getCategoryIndentLevel(
  category: { name: string; parentId?: string; parentIds?: string[] },
  categoriesById: Map<string, { name: string }>
): number {
  if (getCategorySection(category, categoriesById) !== 'Class & Role Specific') return 0;

  return ([category.parentId, ...(category.parentIds ?? [])] as string[])
    .filter(Boolean)
    .some(
      (id: string) =>
        normalizeCategoryName(categoriesById.get(id)?.name ?? '') === 'class role specific'
    )
    ? 1
    : 0;
}

export function parseVersionParts(version: string): number[] {
  return version
    .split('.')
    .map((part) => Number.parseInt(part, 10))
    .filter((part) => Number.isFinite(part));
}

export function compareVersionStrings(a: string, b: string): number {
  const aParts = parseVersionParts(a);
  const bParts = parseVersionParts(b);
  const maxLength = Math.max(aParts.length, bParts.length);
  for (let i = 0; i < maxLength; i++) {
    const aPart = aParts[i] ?? 0;
    const bPart = bParts[i] ?? 0;
    if (aPart !== bPart) return aPart - bPart;
  }
  return a.localeCompare(b);
}

export function getUpdatedState(uiDate: string): 'today' | 'recent' | 'normal' {
  if (!uiDate) return 'normal';
  const updatedAt = new Date(`${uiDate}T00:00:00Z`);
  if (Number.isNaN(updatedAt.getTime())) return 'normal';

  const now = new Date();
  const todayUTC = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
  const updatedUTC = Date.UTC(
    updatedAt.getUTCFullYear(),
    updatedAt.getUTCMonth(),
    updatedAt.getUTCDate()
  );
  const diffDays = Math.floor((todayUTC - updatedUTC) / 86_400_000);
  if (diffDays <= 0) return 'today';
  if (diffDays <= 3) return 'recent';
  return 'normal';
}
