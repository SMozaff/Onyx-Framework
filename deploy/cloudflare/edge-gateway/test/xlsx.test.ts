/**
 * .xlsx reader and writer. The reader is also tested on Excel-style layouts
 * (shared strings, deflated parts) that the Worker's own writer never emits.
 */
import { describe, expect, it } from "vitest";
import { buildXlsx, protectFormulaText, readFirstSheet, XlsxError } from "../src/xlsx";
import { makeZip } from "./helpers/zip";

const SHEET_HEADER = `<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>`;
const SHEET_FOOTER = `</sheetData></worksheet>`;
const WORKBOOK = `<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Staff" sheetId="1" r:id="rId7"/></sheets></workbook>`;
const RELS = `<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId7" Type="worksheet" Target="/xl/worksheets/sheet3.xml"/></Relationships>`;

describe("xlsx writer", () => {
  it("writes text cells that read back exactly, with escaped markup", async () => {
    const bytes = buildXlsx("Report", [
      ["onyx_id", "national_id", "note"],
      ["st26ab0001", "0012345678", `a & b <c> "d" 'e'`],
    ]);
    const sheet = await readFirstSheet(bytes);
    expect(sheet.headers).toEqual(["onyx_id", "national_id", "note"]);
    expect(sheet.rows[0].cells.national_id).toEqual({ text: "0012345678", numeric: false });
    expect(sheet.rows[0].cells.note.text).toBe(`a & b <c> "d" 'e'`);
  });

  it("removes control characters that XML cannot carry", async () => {
    const bytes = buildXlsx("R", [["x"], ["a\u0001b\u000Bc"]]);
    const sheet = await readFirstSheet(bytes);
    expect(sheet.rows[0].cells.x.text).toBe("abc");
  });

  it("protects text that a spreadsheet would read as a formula", async () => {
    expect(protectFormulaText("=1+1")).toBe("'=1+1");
    expect(protectFormulaText("+cmd")).toBe("'+cmd");
    expect(protectFormulaText("-2")).toBe("'-2");
    expect(protectFormulaText("@SUM(A1)")).toBe("'@SUM(A1)");
    expect(protectFormulaText("\tx")).toBe("'\tx");
    expect(protectFormulaText("Plain")).toBe("Plain");
  });

  it("writes formula-looking input as inline text, never as a formula element", async () => {
    const bytes = buildXlsx("R", [["v"], ["=HYPERLINK(\"x\")"]]);
    const sheet = await readFirstSheet(bytes);
    expect(sheet.rows[0].cells.v.text).toBe("'=HYPERLINK(\"x\")");
    expect(sheet.rows[0].cells.v.numeric).toBe(false);
  });
});

describe("xlsx reader", () => {
  it("reads shared strings and deflated parts, as Excel writes them", async () => {
    const shared = `<?xml version="1.0"?><sst><si><t>onyx_id</t></si><si><t>first_name</t></si><si><t>last_name</t></si><si><t>national_id</t></si><si><r><t>Rich </t></r><r><t>text</t></r></si></sst>`;
    const sheet = `${SHEET_HEADER}<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c><c r="C1" t="s"><v>2</v></c><c r="D1" t="s"><v>3</v></c></row>` +
      `<row r="2"><c r="B2" t="s"><v>4</v></c><c r="C2" t="inlineStr"><is><t>Anders &amp; Co</t></is></c><c r="D2" s="1"><v>0012345678</v></c></row>${SHEET_FOOTER}`;
    const bytes = makeZip([
      { name: "xl/workbook.xml", content: WORKBOOK, deflate: true },
      { name: "xl/_rels/workbook.xml.rels", content: RELS, deflate: true },
      { name: "xl/sharedStrings.xml", content: shared, deflate: true },
      { name: "xl/worksheets/sheet3.xml", content: sheet, deflate: true },
    ]);
    const parsed = await readFirstSheet(bytes);
    expect(parsed.headers).toEqual(["onyx_id", "first_name", "last_name", "national_id"]);
    const row = parsed.rows[0];
    expect(row.rowNumber).toBe(2);
    expect(row.cells.first_name.text).toBe("Rich text");
    expect(row.cells.last_name.text).toBe("Anders & Co");
    // A numeric cell is flagged so the national ID rule can reject it.
    expect(row.cells.national_id).toEqual({ text: "0012345678", numeric: true });
  });

  it("reads boolean and error cells as text, not numbers", async () => {
    const sheet = `${SHEET_HEADER}<row r="1"><c r="A1" t="inlineStr"><is><t>flag</t></is></c></row><row r="2"><c r="A2" t="b"><v>1</v></c></row>${SHEET_FOOTER}`;
    const bytes = makeZip([
      { name: "xl/workbook.xml", content: WORKBOOK },
      { name: "xl/_rels/workbook.xml.rels", content: RELS },
      { name: "xl/worksheets/sheet3.xml", content: sheet },
    ]);
    const parsed = await readFirstSheet(bytes);
    expect(parsed.rows[0].cells.flag).toEqual({ text: "TRUE", numeric: false });
  });

  it("rejects bytes that are not a zip archive", async () => {
    await expect(readFirstSheet(new TextEncoder().encode("onyx_id,first_name\n1,2\n"))).rejects.toBeInstanceOf(XlsxError);
  });

  it("refuses an archive whose expanded size exceeds the cap, before inflating it", async () => {
    const huge = Buffer.alloc(9 * 1024 * 1024, 0x41);
    const sheet = `${SHEET_HEADER}<row r="1"><c r="A1" t="inlineStr"><is><t>x</t></is></c></row>${SHEET_FOOTER}`;
    const bytes = makeZip([
      { name: "xl/workbook.xml", content: WORKBOOK },
      { name: "xl/_rels/workbook.xml.rels", content: RELS },
      { name: "xl/worksheets/sheet3.xml", content: sheet },
      { name: "xl/unused.bin", content: huge, deflate: true },
    ]);
    await expect(readFirstSheet(bytes)).rejects.toThrow(/too large/);
  });

  it("reports a workbook with no sheet as an error", async () => {
    const bytes = makeZip([{ name: "xl/workbook.xml", content: `<workbook></workbook>` }]);
    await expect(readFirstSheet(bytes)).rejects.toBeInstanceOf(XlsxError);
  });
});
