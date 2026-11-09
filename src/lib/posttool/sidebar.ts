export const SIDEBAR_DEFAULT = 288;
export const SIDEBAR_MIN = 224;
export const SIDEBAR_MAX = 560;
export const SIDEBAR_STORAGE_KEY = "posttool.sidebarWidth.v1";

export function sidebarWidth(value: number, available: number): number {
  const maximum = Math.max(SIDEBAR_MIN, Math.min(SIDEBAR_MAX, available - 620));
  return Math.round(
    Math.min(
      maximum,
      Math.max(SIDEBAR_MIN, Number.isFinite(value) ? value : SIDEBAR_DEFAULT),
    ),
  );
}

export function storedSidebarWidth(value: string | null): number {
  const width = value === null ? SIDEBAR_DEFAULT : Number(value);
  return Math.max(
    SIDEBAR_MIN,
    Math.min(SIDEBAR_MAX, Number.isFinite(width) ? width : SIDEBAR_DEFAULT),
  );
}
