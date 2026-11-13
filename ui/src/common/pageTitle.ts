const APP_NAME = "Kellnr";

export function formatPageTitle(title: string | undefined): string {
  const trimmed = title?.trim();
  return trimmed ? `${trimmed} - ${APP_NAME}` : APP_NAME;
}

export function setPageTitle(title: string | undefined): void {
  document.title = formatPageTitle(title);
}
