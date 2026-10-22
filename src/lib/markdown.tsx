// Рендер Markdown в React-узлы (без dangerouslySetInnerHTML — так безопаснее и
// не мешает CSP приложения). Поддержаны заголовки, списки (в т.ч. вложенные),
// цитаты, код (строчный и блоками), таблицы, разделители, ссылки, картинки,
// жирный/курсив/зачёркнутый, автоссылки.
import { Fragment, type ReactNode } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';

function link(href: string, children: ReactNode): ReactNode {
  // Разрешаем только безопасные схемы; иначе показываем текст как есть.
  if (!/^(https?:|mailto:)/i.test(href)) {
    return <span>{children}</span>;
  }
  return (
    <a
      className="md-a"
      href={href}
      onClick={(e) => {
        e.preventDefault();
        void openUrl(href).catch(() => {});
      }}
    >
      {children}
    </a>
  );
}

/** Маркер жёсткого переноса строки из `<br>`. */
const BR = '\u0001';

/** Раскрывает основные HTML-сущности. `&amp;` — последним. */
function decodeEntities(s: string): string {
  return s
    .replace(/&nbsp;/gi, ' ')
    .replace(/&lt;/gi, '<')
    .replace(/&gt;/gi, '>')
    .replace(/&quot;/gi, '"')
    .replace(/&#0*39;|&apos;|&#x27;/gi, "'")
    .replace(/&mdash;/gi, '—')
    .replace(/&ndash;/gi, '–')
    .replace(/&hellip;/gi, '…')
    .replace(/&rsquo;/gi, '’')
    .replace(/&lsquo;/gi, '‘')
    .replace(/&rdquo;/gi, '”')
    .replace(/&ldquo;/gi, '“')
    .replace(/&laquo;/gi, '«')
    .replace(/&raquo;/gi, '»')
    .replace(/&copy;/gi, '©')
    .replace(/&reg;/gi, '®')
    .replace(/&trade;/gi, '™')
    .replace(/&amp;/gi, '&');
}

/**
 * Приводит HTML-вставки к Markdown. Modrinth отдаёт `body` как Markdown, но
 * авторы часто пишут `<br>`, `<hr>`, `<p>`, `<b>` — без этого они видны как
 * текст. Неизвестные теги (например, `<player>`) не трогаем.
 */
function htmlToMarkdown(src: string): string {
  let s = decodeEntities(src).replace(/<!--[\s\S]*?-->/g, '');

  // Блок кода: <pre><code>…</code></pre>
  s = s.replace(
    /<pre\b[^>]*>\s*<code\b[^>]*>([\s\S]*?)<\/code\s*>\s*<\/pre\s*>/gi,
    (_m, code: string) => `\n\n\`\`\`\n${code.replace(/<\/?[^>]+>/g, '')}\n\`\`\`\n\n`,
  );

  // <a href="…">текст</a>
  s = s.replace(
    /<a\b[^>]*\bhref\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))[^>]*>([\s\S]*?)<\/a\s*>/gi,
    (_m, dq: string, sq: string, bare: string, text: string) => {
      const href = (dq ?? sq ?? bare ?? '').trim();
      const label = text.replace(/<[^>]+>/g, '').trim();
      return href ? `[${label || href}](${href})` : label;
    },
  );

  // <img src="…" alt="…">
  s = s.replace(/<img\b[^>]*>/gi, (tag: string) => {
    const attr = (name: string) => {
      const m = new RegExp(
        `\\b${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s>]+))`,
        'i',
      ).exec(tag);
      return (m?.[1] ?? m?.[2] ?? m?.[3] ?? '').trim();
    };
    const url = attr('src');
    return url ? `![${attr('alt')}](${url})` : '';
  });

  // Заголовки <h1>…</h1>.
  s = s.replace(
    /<h([1-6])\b[^>]*>([\s\S]*?)<\/h\1\s*>/gi,
    (_m, n: string, text: string) => {
      const t = text.replace(/<[^>]+>/g, '').trim();
      return t ? `\n\n${'#'.repeat(Number(n))} ${t}\n\n` : '\n\n';
    },
  );

  // Парные выделения.
  s = s.replace(/<\/?(?:b|strong)\b[^>]*>/gi, '**');
  s = s.replace(/<\/?(?:i|em)\b[^>]*>/gi, '*');
  s = s.replace(/<\/?(?:del|s|strike)\b[^>]*>/gi, '~~');
  s = s.replace(/<\/?code\b[^>]*>/gi, '`');

  // Списки.
  s = s.replace(/<li\b[^>]*>/gi, '\n- ');
  s = s.replace(/<\/li\s*>/gi, '');

  // Таблицы — хотя бы по ячейке в строке.
  s = s.replace(/<\/t[dh]\s*>/gi, ' | ');
  s = s.replace(/<t[dh]\b[^>]*>/gi, '');
  s = s.replace(/<\/?tr\b[^>]*>/gi, '\n');

  // Переносы и разделители.
  s = s.replace(/<br\b[^>]*>/gi, BR);
  s = s.replace(/<hr\b[^>]*>/gi, '\n\n---\n\n');

  // Блочные теги.
  s = s.replace(
    /<\/?(?:p|div|section|article|header|footer|main|aside|nav|ul|ol|table|thead|tbody|tfoot|blockquote|pre|details|summary|figure|figcaption|center)\b[^>]*>/gi,
    '\n\n',
  );

  // Известные, но не нужные теги вырезаем.
  s = s.replace(
    /<\/?(?:span|font|small|big|sub|sup|u|tt|kbd|mark|wbr|abbr|cite|q|iframe|video|audio|source|picture|colgroup|col|caption)\b[^>]*>/gi,
    '',
  );

  s = decodeEntities(s);
  // Если теги были закодированы дважды: &amp;lt;br&amp;gt;.
  s = s.replace(/<br\b[^>]*>/gi, BR).replace(/<hr\b[^>]*>/gi, '\n\n---\n\n');
  s = s.replace(/[ \t]+$/gm, '').replace(/\n{3,}/g, '\n\n');
  return s;
}

/** Короткий текст без HTML-мусора — для карточек и подзаголовков. */
export function plainText(src: string): string {
  return decodeEntities(src)
    .replace(/<br\b[^>]*>/gi, ' ')
    .replace(/<hr\b[^>]*>/gi, ' ')
    .replace(/<\/?[a-zA-Z][^>]*>/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

/** Сноски Markdown: `[1]: https://…` → 1 → адрес. */
type Refs = Map<string, string>;

/** Позиция парной скобки: `[` на `start`, с учётом вложенных. */
function matchBracket(src: string, start: number): number {
  let depth = 0;
  for (let i = start; i < src.length; i++) {
    const c = src[i];
    if (c === '\\') {
      i++;
      continue;
    }
    if (c === '[') depth++;
    else if (c === ']') {
      depth--;
      if (depth === 0) return i;
    }
  }
  return -1;
}

/** Позиция парной скобки: `(` на `start`; в адресах бывают свои скобки. */
function matchParen(src: string, start: number): number {
  let depth = 0;
  for (let i = start; i < src.length; i++) {
    const c = src[i];
    if (c === '\\') {
      i++;
      continue;
    }
    if (c === '(') depth++;
    else if (c === ')') {
      depth--;
      if (depth === 0) return i;
    }
  }
  return -1;
}

/**
 * Ссылка или картинка в позиции `i`: `[т](url)`, `[т][1]`, `[т]`,
 * `![alt](url)`, `![alt][1]`. Текст ссылки может содержать картинку —
 * так авторы сборок делают кнопки-картинки.
 */
function parseLink(
  src: string,
  i: number,
  refs?: Refs,
): { node: ReactNode; len: number } | null {
  const image = src[i] === '!' && src[i + 1] === '[';
  const open = image ? i + 1 : i;
  if (src[open] !== '[') return null;
  const close = matchBracket(src, open);
  if (close < 0) return null;
  const label = src.slice(open + 1, close);
  if (!label.trim()) return null;

  let url = '';
  let end = close + 1;
  if (src[end] === '(') {
    const p = matchParen(src, end);
    if (p < 0) return null;
    url = src
      .slice(end + 1, p)
      .trim()
      .replace(/\s+(?:"[^"]*"|'[^']*')$/, '')
      .trim();
    end = p + 1;
  } else if (src[end] === '[') {
    const c2 = matchBracket(src, end);
    if (c2 < 0) return null;
    const id = (src.slice(end + 1, c2).trim() || label.trim()).toLowerCase();
    url = refs?.get(id) ?? '';
    end = c2 + 1;
  } else if (refs) {
    url = refs.get(label.trim().toLowerCase()) ?? '';
  }
  if (!url) return null;

  if (image) {
    if (!/^https?:/i.test(url)) return null;
    return {
      node: <img className="md-img" src={url} alt={label} loading="lazy" />,
      len: end - i,
    };
  }
  return { node: link(url, inline(label, refs)), len: end - i };
}

/** Разбирает одну строку на узлы: код, ссылки, картинки, выделения. */
function inline(src: string, refs?: Refs): ReactNode[] {
  const out: ReactNode[] = [];
  let buf = '';
  let i = 0;
  let k = 0;

  const flush = () => {
    if (buf) {
      out.push(buf);
      buf = '';
    }
  };
  const push = (n: ReactNode) => {
    flush();
    out.push(<Fragment key={'i' + k++}>{n}</Fragment>);
  };

  while (i < src.length) {
    const rest = src.slice(i);
    let node: ReactNode | null = null;
    let len = 0;

    // Жёсткий перенос из <br>.
    if (src[i] === BR) {
      push(<br />);
      i++;
      continue;
    }

    // `код`
    if (src[i] === '`') {
      const m = /^`([^`]+)`/.exec(rest);
      if (m) {
        node = <code className="md-ic">{m[1]}</code>;
        len = m[0].length;
      }
    }

    // ![alt](url), ![alt][1], [text](url), [text][1], [text]
    if (!node && (src[i] === '[' || (src[i] === '!' && src[i + 1] === '['))) {
      const hit = parseLink(src, i, refs);
      if (hit) {
        node = hit.node;
        len = hit.len;
      }
    }

    // <https://autolink>
    if (!node && src[i] === '<') {
      const m = /^<((?:https?:\/\/|mailto:)[^>\s]+)>/.exec(rest);
      if (m) {
        node = link(m[1], m[1].replace(/^mailto:/, ''));
        len = m[0].length;
      }
    }

    // ~~зачёркнутый~~
    if (!node && src.startsWith('~~', i)) {
      const end = src.indexOf('~~', i + 2);
      if (end > i + 2) {
        node = <del>{inline(src.slice(i + 2, end), refs)}</del>;
        len = end + 2 - i;
      }
    }

    // **жирный**, *курсив*, __жирный__, _курсив_, ***жирный курсив***
    if (!node) {
      for (const marker of ['***', '**', '__', '*', '_']) {
        if (!src.startsWith(marker, i)) continue;
        if (marker === '_' && i > 0 && /[A-Za-z0-9]/.test(src[i - 1])) continue;
        const end = src.indexOf(marker, i + marker.length);
        if (end <= i + marker.length) continue;
        const inner = src.slice(i + marker.length, end);
        if (inner.includes('\n')) continue;
        node =
          marker.length === 3 ? (
            <strong>
              <em>{inline(inner, refs)}</em>
            </strong>
          ) : marker.length === 2 ? (
            <strong>{inline(inner, refs)}</strong>
          ) : (
            <em>{inline(inner, refs)}</em>
          );
        len = end + marker.length - i;
        break;
      }
    }

    if (node) {
      push(node);
      i += len;
      continue;
    }
    buf += src[i];
    i++;
  }
  flush();
  return out;
}

function splitRow(line: string): string[] {
  let t = line.trim();
  if (t.startsWith('|')) t = t.slice(1);
  if (t.endsWith('|')) t = t.slice(0, -1);
  return t.split('|').map((c) => c.trim());
}

function Heading({ level, children }: { level: number; children: ReactNode }) {
  const cls = 'md-h';
  if (level === 1) return <h1 className={cls}>{children}</h1>;
  if (level === 2) return <h2 className={cls}>{children}</h2>;
  if (level === 3) return <h3 className={cls}>{children}</h3>;
  if (level === 4) return <h4 className={cls}>{children}</h4>;
  if (level === 5) return <h5 className={cls}>{children}</h5>;
  return <h6 className={cls}>{children}</h6>;
}

interface ListItem {
  depth: number;
  text: string;
}

function List({ ordered, items, refs }: { ordered: boolean; items: ListItem[]; refs?: Refs }) {
  const Tag = ordered ? 'ol' : 'ul';
  return (
    <Tag className="md-list">
      {items.map((it, i) => (
        <li key={i} style={{ marginLeft: it.depth * 16 }}>
          {inline(it.text, refs)}
        </li>
      ))}
    </Tag>
  );
}

/** Разбор Markdown на блоки и отрисовка. */
export function Markdown({ text }: { text: string }) {
  const lines = htmlToMarkdown(text).replace(/\r\n?/g, '\n').split('\n');
  const refs: Refs = new Map();
  // Сноски `[1]: https://…` собираем заранее и из текста убираем — так ссылки
  // вида `[текст][1]` работают, даже если сноска описана ниже.
  for (let n = 0; n < lines.length; n++) {
    const m = /^\s{0,3}\[([^\]]+)\]:\s*<?([^\s>]+)>?/.exec(lines[n]);
    if (m) {
      refs.set(m[1].trim().toLowerCase(), m[2]);
      lines[n] = '';
    }
  }
  const blocks: ReactNode[] = [];
  let i = 0;
  let k = 0;
  const next = (n: ReactNode) => blocks.push(<Fragment key={'b' + k++}>{n}</Fragment>);

  while (i < lines.length) {
    const line = lines[i];
    const trimmed = line.trim();

    if (!trimmed) {
      i++;
      continue;
    }

    // Блок кода ```lang … ```
    const fence = /^```(.*)$/.exec(trimmed);
    if (fence) {
      const lang = fence[1].trim();
      const code: string[] = [];
      i++;
      while (i < lines.length && !/^```/.test(lines[i].trim())) {
        code.push(lines[i]);
        i++;
      }
      i++;
      next(
        <pre className="md-pre">
          <code data-lang={lang || undefined}>{code.join('\n')}</code>
        </pre>,
      );
      continue;
    }

    // Заголовок
    const h = /^(#{1,6})\s+(.*)$/.exec(trimmed);
    if (h) {
      next(<Heading level={h[1].length}>{inline(h[2], refs)}</Heading>);
      i++;
      continue;
    }

    // Разделитель
    if (/^([-*_])(\s*\1){2,}$/.test(trimmed)) {
      next(<hr className="md-hr" />);
      i++;
      continue;
    }

    // Цитата
    if (trimmed.startsWith('>')) {
      const q: string[] = [];
      while (i < lines.length && lines[i].trim().startsWith('>')) {
        q.push(lines[i].trim().replace(/^>\s?/, ''));
        i++;
      }
      next(<blockquote className="md-quote">{inline(q.join(' '), refs)}</blockquote>);
      continue;
    }

    // Таблица | a | b |
    if (
      trimmed.includes('|') &&
      i + 1 < lines.length &&
      lines[i + 1].includes('-') &&
      /^\s*\|?[\s:|-]+\|[\s:|-]*$/.test(lines[i + 1])
    ) {
      const head = splitRow(trimmed);
      i += 2;
      const rows: string[][] = [];
      while (i < lines.length && lines[i].trim().includes('|')) {
        rows.push(splitRow(lines[i]));
        i++;
      }
      next(
        <div className="md-tablewrap">
          <table className="md-table">
            <thead>
              <tr>
                {head.map((c, n) => (
                  <th key={n}>{inline(c, refs)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((r, n) => (
                <tr key={n}>
                  {r.map((c, m) => (
                    <td key={m}>{inline(c, refs)}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>,
      );
      continue;
    }

    // Список (маркированный или нумерованный, с вложенностью)
    if (/^\s*([-*+]|\d+[.)])\s+/.test(line)) {
      const ordered = /^\s*\d+[.)]\s+/.test(line);
      const items: ListItem[] = [];
      while (i < lines.length) {
        const m = /^(\s*)([-*+]|\d+[.)])\s+(.*)$/.exec(lines[i]);
        if (!m) break;
        const depth = Math.floor(m[1].replace(/\t/g, '    ').length / 2);
        items.push({ depth, text: m[3] });
        i++;
        while (
          i < lines.length &&
          lines[i].trim() &&
          !/^\s*([-*+]|\d+[.)])\s+/.test(lines[i]) &&
          /^\s{2,}/.test(lines[i])
        ) {
          items[items.length - 1].text += ' ' + lines[i].trim();
          i++;
        }
      }
      next(<List ordered={ordered} items={items} refs={refs} />);
      continue;
    }

    // Абзац
    const p: string[] = [trimmed];
    i++;
    while (
      i < lines.length &&
      lines[i].trim() &&
      !/^(#{1,6}\s|>|```)/.test(lines[i].trim()) &&
      !/^\s*([-*+]|\d+[.)])\s+/.test(lines[i])
    ) {
      p.push(lines[i].trim());
      i++;
    }
    next(<p className="md-p">{inline(p.join(' '), refs)}</p>);
  }

  return <div className="md">{blocks}</div>;
}
