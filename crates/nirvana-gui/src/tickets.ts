const ticketBaseUrl =
  import.meta.env.VITE_NIRVANA_TICKET_BASE_URL ?? "https://acme.atlassian.net/browse";

export const ticketUrlForKey = (ticketKey: string) => {
  const baseUrl = ticketBaseUrl.replace(/\/$/, "");
  return `${baseUrl}/${encodeURIComponent(ticketKey)}`;
};
