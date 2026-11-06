// RFC 4180-style CSV, including quoted commas, CRLF and embedded newlines.
export function iterationData(text: string): Record<string, string>[] {
  if (!text.trim()) return [{}];
  if (text.length > 4 * 1024 * 1024) throw new Error("运行数据超过 4 MiB");
  if (text.trim().startsWith("[")) {
    const data = JSON.parse(text);
    if (!Array.isArray(data) || data.length > 10000)
      throw new Error("JSON 数据需要数组且最多 10,000 行");
    return data.map((row) => {
      if (!row || typeof row !== "object" || Array.isArray(row))
        throw new Error("每行运行数据需要对象");
      return Object.fromEntries(
        Object.entries(row).map(([k, v]) => [
          k,
          typeof v === "object" ? JSON.stringify(v) : String(v ?? ""),
        ]),
      );
    });
  }
  const rows: string[][] = [];
  let row: string[] = [],
    cell = "",
    quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i]!;
    if (c === '"') {
      if (quoted && text[i + 1] === '"') {
        cell += '"';
        i++;
      } else if (!quoted && cell.length)
        throw new Error("CSV 引号必须位于字段开头");
      else quoted = !quoted;
    } else if (c === "," && !quoted) {
      row.push(cell);
      cell = "";
    } else if ((c === "\n" || c === "\r") && !quoted) {
      row.push(cell);
      if (row.some(Boolean)) rows.push(row);
      row = [];
      cell = "";
      if (c === "\r" && text[i + 1] === "\n") i++;
    } else cell += c;
    if (rows.length > 10001) throw new Error("运行数据最多 10,000 行");
  }
  if (quoted) throw new Error("CSV 引号未闭合");
  row.push(cell);
  if (row.some(Boolean)) rows.push(row);
  const header = rows.shift()?.map((s) => s.replace(/^\uFEFF/, "")) ?? [];
  if (
    !header.length ||
    header.some((k) => !k) ||
    new Set(header).size !== header.length
  )
    throw new Error("CSV 列名不能为空或重复");
  return rows.map((values) => {
    if (values.length !== header.length)
      throw new Error("CSV 数据列数与表头不一致");
    return Object.fromEntries(header.map((k, i) => [k, values[i] ?? ""]));
  });
}
