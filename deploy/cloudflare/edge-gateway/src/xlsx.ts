/**
 * Minimal .xlsx reader and writer for the Worker. Dependency-free and limited
 * to what the staff import and the creation report need.
 *
 * Reader: first worksheet only, shared strings and inline strings, stored and
 * deflate entries (deflate via the platform DecompressionStream). Zip64,
 * encryption and multi-disk archives are rejected. Expanded size is capped so
 * a small compressed bomb cannot exhaust Worker memory.
 *
 * Writer: one worksheet, every cell an inline string formatted as text, stored
 * (uncompressed) zip entries. Text cells that begin with a spreadsheet formula
 * trigger character are prefixed with an apostrophe so Excel shows them as
 * literal text (formula-injection protection).
 */

export class XlsxError extends Error {}

export interface XlsxCell {
  text: string;
  /** True when the cell was stored as a number (or date/bool), not as text. */
  numeric: boolean;
}

export interface XlsxSheet {
  /** Lowercased, trimmed header names in column order. */
  headers: string[];
  rows: Array<{ rowNumber: number; cells: Record<string, XlsxCell> }>;
}

const MAX_ZIP_ENTRIES = 64;
const MAX_EXPANDED_BYTES = 8 * 1024 * 1024;
const LOCAL_SIG = 0x04034b50;
const CENTRAL_SIG = 0x02014b50;
const EOCD_SIG = 0x06054b50;

function u16(b: Uint8Array, o: number): number {
  return b[o] | (b[o + 1] << 8);
}
function u32(b: Uint8Array, o: number): number {
  return (b[o] | (b[o + 1] << 8) | (b[o + 2] << 16) | (b[o + 3] << 24)) >>> 0;
}

const utf8 = new TextDecoder("utf-8");

function decodeXmlEntities(value: string): string {
  return value
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&#x([0-9a-fA-F]+);/g, (_, hex) => String.fromCodePoint(parseInt(hex, 16)))
    .replace(/&#(\d+);/g, (_, dec) => String.fromCodePoint(parseInt(dec, 10)))
    .replace(/&amp;/g, "&");
}

async function inflateRaw(data: Uint8Array): Promise<Uint8Array> {
  if (typeof DecompressionStream === "undefined") throw new XlsxError("deflate unsupported");
  const stream = new Blob([data as unknown as BlobPart]).stream().pipeThrough(new DecompressionStream("deflate-raw") as unknown as TransformStream<Uint8Array, Uint8Array>);
  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    total += value.byteLength;
    if (total > MAX_EXPANDED_BYTES) throw new XlsxError("workbook too large");
    chunks.push(value);
  }
  const out = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return out;
}

async function readZipEntries(bytes: Uint8Array): Promise<Map<string, Uint8Array>> {
  // End-of-central-directory record sits within the last 64 KiB + 22 bytes.
  let eocd = -1;
  for (let i = bytes.length - 22; i >= Math.max(0, bytes.length - 65557); i--) {
    if (u32(bytes, i) === EOCD_SIG) { eocd = i; break; }
  }
  if (eocd < 0) throw new XlsxError("not a zip archive");
  const count = u16(bytes, eocd + 10);
  const cdOffset = u32(bytes, eocd + 16);
  if (count === 0xffff || cdOffset === 0xffffffff) throw new XlsxError("zip64 is not supported");
  if (count > MAX_ZIP_ENTRIES) throw new XlsxError("too many zip entries");

  const entries = new Map<string, Uint8Array>();
  let p = cdOffset;
  let declaredTotal = 0;
  for (let n = 0; n < count; n++) {
    if (p + 46 > bytes.length || u32(bytes, p) !== CENTRAL_SIG) throw new XlsxError("corrupt central directory");
    const method = u16(bytes, p + 10);
    const compressedSize = u32(bytes, p + 20);
    const uncompressedSize = u32(bytes, p + 24);
    const nameLength = u16(bytes, p + 28);
    const extraLength = u16(bytes, p + 30);
    const commentLength = u16(bytes, p + 32);
    const localOffset = u32(bytes, p + 42);
    const name = utf8.decode(bytes.subarray(p + 46, p + 46 + nameLength));
    p += 46 + nameLength + extraLength + commentLength;

    declaredTotal += uncompressedSize;
    if (declaredTotal > MAX_EXPANDED_BYTES) throw new XlsxError("workbook too large");
    if (localOffset + 30 > bytes.length || u32(bytes, localOffset) !== LOCAL_SIG) throw new XlsxError("corrupt local header");
    const start = localOffset + 30 + u16(bytes, localOffset + 26) + u16(bytes, localOffset + 28);
    const data = bytes.subarray(start, start + compressedSize);
    if (data.length !== compressedSize) throw new XlsxError("truncated entry");
    if (method === 0) entries.set(name, data);
    else if (method === 8) entries.set(name, await inflateRaw(data));
    else throw new XlsxError("unsupported zip compression");
  }
  return entries;
}

function xmlText(fragment: string): string {
  let out = "";
  const re = /<t(?:\s[^>]*)?>([\s\S]*?)<\/t>/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(fragment))) out += decodeXmlEntities(m[1]);
  return out;
}

function columnIndex(ref: string): number {
  const letters = ref.match(/^[A-Z]+/i)?.[0] ?? "";
  let index = 0;
  for (const ch of letters.toUpperCase()) index = index * 26 + (ch.charCodeAt(0) - 64);
  return index;
}

/** Reads the first worksheet of an .xlsx workbook. Throws XlsxError when unreadable. */
export async function readFirstSheet(bytes: Uint8Array): Promise<XlsxSheet> {
  if (bytes.length < 4 || bytes[0] !== 0x50 || bytes[1] !== 0x4b) throw new XlsxError("not an xlsx workbook");
  const entries = await readZipEntries(bytes);
  const workbook = entries.get("xl/workbook.xml");
  if (!workbook) throw new XlsxError("workbook has no sheet");
  const workbookXml = utf8.decode(workbook);
  const sheetTag = workbookXml.match(/<sheet\b[^>]*>/);
  const relId = sheetTag?.[0].match(/r:id="([^"]+)"/)?.[1];
  if (!relId) throw new XlsxError("workbook has no sheet");

  const rels = entries.get("xl/_rels/workbook.xml.rels");
  if (!rels) throw new XlsxError("workbook relationships missing");
  let target: string | undefined;
  for (const tag of utf8.decode(rels).match(/<Relationship\b[^>]*>/g) ?? []) {
    if (new RegExp(`\\bId="${relId}"`).test(tag)) target = tag.match(/Target="([^"]+)"/)?.[1];
  }
  if (!target) throw new XlsxError("sheet relationship missing");
  const sheetPath = target.startsWith("/") ? target.slice(1) : target.startsWith("xl/") ? target : `xl/${target}`;
  const sheetBytes = entries.get(sheetPath);
  if (!sheetBytes) throw new XlsxError("worksheet missing");

  const sharedStrings: string[] = [];
  const sst = entries.get("xl/sharedStrings.xml");
  if (sst) {
    for (const si of utf8.decode(sst).match(/<si\b[^>]*>[\s\S]*?<\/si>/g) ?? []) sharedStrings.push(xmlText(si));
  }

  const sheetXml = utf8.decode(sheetBytes);
  const rowMatches = sheetXml.match(/<row\b[^>]*>[\s\S]*?<\/row>|<row\b[^>]*\/>/g) ?? [];
  const parsedRows: Array<{ rowNumber: number; cells: Map<number, XlsxCell> }> = [];
  let sequential = 0;
  for (const rowXml of rowMatches) {
    sequential++;
    const rowNumber = Number(rowXml.match(/<row\b[^>]*\br="(\d+)"/)?.[1] ?? sequential);
    const cells = new Map<number, XlsxCell>();
    const cellRe = /<c\b([^>]*?)(?:\/>|>([\s\S]*?)<\/c>)/g;
    let c: RegExpExecArray | null;
    while ((c = cellRe.exec(rowXml))) {
      const attrs = c[1];
      const ref = attrs.match(/\br="([A-Z]+\d+)"/i)?.[1] ?? "";
      const type = attrs.match(/\bt="([^"]+)"/)?.[1] ?? "n";
      const inner = c[2] ?? "";
      const valueMatch = inner.match(/<v>([\s\S]*?)<\/v>/);
      const rawValue = valueMatch ? decodeXmlEntities(valueMatch[1]) : "";
      let cell: XlsxCell;
      if (type === "s") cell = { text: sharedStrings[Number(rawValue)] ?? "", numeric: false };
      else if (type === "inlineStr") cell = { text: xmlText(inner), numeric: false };
      else if (type === "str") cell = { text: rawValue, numeric: false };
      else if (type === "b") cell = { text: rawValue === "1" ? "TRUE" : "FALSE", numeric: false };
      else if (type === "e") cell = { text: rawValue, numeric: false };
      else cell = { text: rawValue, numeric: rawValue !== "" };
      if (ref) cells.set(columnIndex(ref), cell);
    }
    parsedRows.push({ rowNumber, cells });
  }
  if (parsedRows.length === 0) return { headers: [], rows: [] };

  const headerRow = parsedRows[0];
  const columns = [...headerRow.cells.keys()].sort((a, b) => a - b);
  const headers = columns.map((index) => headerRow.cells.get(index)!.text.trim().toLowerCase());
  const rows = parsedRows.slice(1).map((row) => {
    const named: Record<string, XlsxCell> = {};
    columns.forEach((index, position) => {
      const name = headers[position];
      if (name) named[name] = row.cells.get(index) ?? { text: "", numeric: false };
    });
    return { rowNumber: row.rowNumber, cells: named };
  });
  return { headers: headers.filter(Boolean), rows };
}

// ---- Writer -------------------------------------------------------------

const FORMULA_TRIGGER = /^[=+\-@\t\r]/;
// XML 1.0 forbids these control characters even when escaped.
const INVALID_XML_CHARS = /[\u0000-\u0008\u000B\u000C\u000E-\u001F￾￿]/g;

/** Text that a spreadsheet would read as a formula gets a leading apostrophe. */
export function protectFormulaText(value: string): string {
  return FORMULA_TRIGGER.test(value) ? `'${value}` : value;
}

function escapeXml(value: string): string {
  return value
    .replace(INVALID_XML_CHARS, "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

function columnLetters(index: number): string {
  let letters = "";
  let n = index + 1;
  while (n > 0) {
    const rem = (n - 1) % 26;
    letters = String.fromCharCode(65 + rem) + letters;
    n = Math.floor((n - 1) / 26);
  }
  return letters;
}

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(data: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of data) crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}

function storeZip(files: Array<{ name: string; data: Uint8Array }>): Uint8Array {
  const encoder = new TextEncoder();
  const parts: Uint8Array[] = [];
  const central: Uint8Array[] = [];
  let offset = 0;
  const dosTime = 0;
  const dosDate = (2026 - 1980) << 9 | (1 << 5) | 1;
  for (const file of files) {
    const name = encoder.encode(file.name);
    const crc = crc32(file.data);
    const local = new DataView(new ArrayBuffer(30));
    local.setUint32(0, LOCAL_SIG, true);
    local.setUint16(4, 20, true);
    local.setUint16(6, 0, true);
    local.setUint16(8, 0, true);
    local.setUint16(10, dosTime, true);
    local.setUint16(12, dosDate, true);
    local.setUint32(14, crc, true);
    local.setUint32(18, file.data.length, true);
    local.setUint32(22, file.data.length, true);
    local.setUint16(26, name.length, true);
    local.setUint16(28, 0, true);
    parts.push(new Uint8Array(local.buffer), name, file.data);

    const dir = new DataView(new ArrayBuffer(46));
    dir.setUint32(0, CENTRAL_SIG, true);
    dir.setUint16(4, 20, true);
    dir.setUint16(6, 20, true);
    dir.setUint16(8, 0, true);
    dir.setUint16(10, 0, true);
    dir.setUint16(12, dosTime, true);
    dir.setUint16(14, dosDate, true);
    dir.setUint32(16, crc, true);
    dir.setUint32(20, file.data.length, true);
    dir.setUint32(24, file.data.length, true);
    dir.setUint16(28, name.length, true);
    dir.setUint32(42, offset, true);
    central.push(new Uint8Array(dir.buffer), name);
    offset += 30 + name.length + file.data.length;
  }
  const centralSize = central.reduce((sum, part) => sum + part.length, 0);
  const eocd = new DataView(new ArrayBuffer(22));
  eocd.setUint32(0, EOCD_SIG, true);
  eocd.setUint16(8, files.length, true);
  eocd.setUint16(10, files.length, true);
  eocd.setUint32(12, centralSize, true);
  eocd.setUint32(16, offset, true);
  const all = [...parts, ...central, new Uint8Array(eocd.buffer)];
  const out = new Uint8Array(all.reduce((sum, part) => sum + part.length, 0));
  let pos = 0;
  for (const part of all) {
    out.set(part, pos);
    pos += part.length;
  }
  return out;
}

/**
 * Builds a one-sheet workbook. Every cell is written as an inline text cell
 * in a text-formatted column, so numbers such as national IDs keep their
 * leading zeros and nothing is evaluated as a formula.
 */
export type XlsxWriteCell = string | { text: string; numeric: true };

export function buildXlsx(sheetName: string, rows: XlsxWriteCell[][]): Uint8Array {
  const sheetRows = rows.map((cells, r) => {
    const body = cells.map((value, c) => {
      const ref = `${columnLetters(c)}${r + 1}`;
      if (typeof value !== "string") {
        // Numeric cells exist only for tests of the rejection rule; the Worker never writes them.
        return `<c r="${ref}"><v>${escapeXml(value.text)}</v></c>`;
      }
      const text = escapeXml(protectFormulaText(value));
      return `<c r="${ref}" t="inlineStr" s="0"><is><t xml:space="preserve">${text}</t></is></c>`;
    }).join("");
    return `<row r="${r + 1}">${body}</row>`;
  }).join("");
  const width = rows.reduce((max, cells) => Math.max(max, cells.length), 1);
  const dimension = `A1:${columnLetters(width - 1)}${Math.max(rows.length, 1)}`;
  const files = [
    {
      name: "[Content_Types].xml",
      data: `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` +
        `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">` +
        `<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>` +
        `<Default Extension="xml" ContentType="application/xml"/>` +
        `<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>` +
        `<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>` +
        `<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>` +
        `</Types>`,
    },
    {
      name: "_rels/.rels",
      data: `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` +
        `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">` +
        `<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>` +
        `</Relationships>`,
    },
    {
      name: "xl/workbook.xml",
      data: `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` +
        `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">` +
        `<sheets><sheet name="${escapeXml(sheetName).slice(0, 31)}" sheetId="1" r:id="rId1"/></sheets></workbook>`,
    },
    {
      name: "xl/_rels/workbook.xml.rels",
      data: `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` +
        `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">` +
        `<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>` +
        `<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>` +
        `</Relationships>`,
    },
    {
      name: "xl/styles.xml",
      data: `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` +
        `<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">` +
        `<fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts>` +
        `<fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills>` +
        `<borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders>` +
        `<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>` +
        `<cellXfs count="1"><xf numFmtId="49" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/></cellXfs>` +
        `<cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>` +
        `</styleSheet>`,
    },
    {
      name: "xl/worksheets/sheet1.xml",
      data: `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` +
        `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">` +
        `<dimension ref="${dimension}"/>` +
        `<sheetData>${sheetRows}</sheetData></worksheet>`,
    },
  ].map((file) => ({ name: file.name, data: new TextEncoder().encode(file.data) }));
  return storeZip(files);
}
