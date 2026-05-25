export const formatDateTimeInput = (date: Date) => {
  const offsetMs = date.getTimezoneOffset() * 60 * 1000;
  return new Date(date.getTime() - offsetMs).toISOString().slice(0, 16);
};

export const parseDateTimeInput = (value: string) => {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
};

export const formatDateInput = (date: Date) => {
  const year = date.getFullYear();
  const month = (date.getMonth() + 1).toString().padStart(2, "0");
  const day = date.getDate().toString().padStart(2, "0");
  return `${year}-${month}-${day}`;
};

export const parseDateInput = (value: string) => {
  const match = value.trim().match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (!match) return null;

  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  const date = new Date(year, month - 1, day);

  if (
    date.getFullYear() !== year ||
    date.getMonth() !== month - 1 ||
    date.getDate() !== day
  ) {
    return null;
  }

  date.setHours(0, 0, 0, 0);
  return date;
};

export type TimeParts = {
  hours: number;
  minutes: number;
  seconds: number;
};

export const formatTimeInput = (date: Date) =>
  `${date.getHours().toString().padStart(2, "0")}:${date
    .getMinutes()
    .toString()
    .padStart(2, "0")}`;

export const parseTimeParts = (value: string): TimeParts | null => {
  const match = value.trim().match(/^(\d{1,2}):(\d{2})(?::(\d{2}))?$/);
  if (!match) return null;

  const hours = Number(match[1]);
  const minutes = Number(match[2]);
  const seconds = match[3] ? Number(match[3]) : 0;

  if (
    !Number.isInteger(hours) ||
    !Number.isInteger(minutes) ||
    !Number.isInteger(seconds) ||
    hours < 0 ||
    hours > 23 ||
    minutes < 0 ||
    minutes > 59 ||
    seconds < 0 ||
    seconds > 59
  ) {
    return null;
  }

  return { hours, minutes, seconds };
};

export const formatTimeParts = (hours: number, minutes: number) =>
  `${hours.toString().padStart(2, "0")}:${minutes
    .toString()
    .padStart(2, "0")}`;

export const formatDurationInput = (durationMs: number) => {
  const totalMinutes = Math.max(0, Math.round(durationMs / 60000));
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;

  if (hours === 0) {
    return `${minutes}m`;
  }

  if (minutes === 0) {
    return `${hours}h`;
  }

  return `${hours}h ${minutes}m`;
};

export const parseDurationInput = (value: string) => {
  const normalized = value.trim().toLowerCase();
  if (!normalized) return null;

  const clockMatch = normalized.match(/^(\d{1,3}):(\d{2})$/);
  if (clockMatch) {
    const hours = Number(clockMatch[1]);
    const minutes = Number(clockMatch[2]);
    if (!Number.isInteger(hours) || !Number.isInteger(minutes) || minutes > 59) {
      return null;
    }
    return (hours * 60 + minutes) * 60 * 1000;
  }

  const unitMatch = normalized.match(/^(?:(\d+)h)?\s*(?:(\d+)m)?$/);
  if (unitMatch && (unitMatch[1] || unitMatch[2])) {
    const hours = unitMatch[1] ? Number(unitMatch[1]) : 0;
    const minutes = unitMatch[2] ? Number(unitMatch[2]) : 0;
    if (!Number.isInteger(hours) || !Number.isInteger(minutes)) return null;
    return (hours * 60 + minutes) * 60 * 1000;
  }

  const minutes = Number(normalized);
  if (Number.isInteger(minutes) && minutes >= 0) {
    return minutes * 60 * 1000;
  }

  return null;
};

export const wrapTimePart = (value: number, maxExclusive: number) =>
  ((value % maxExclusive) + maxExclusive) % maxExclusive;

export const applyTimeParts = (date: Date, time: TimeParts) => {
  const nextDate = new Date(date);
  nextDate.setHours(time.hours, time.minutes, time.seconds, 0);
  return nextDate;
};

export const applyDateAndTimeParts = (date: Date, time: TimeParts) =>
  applyTimeParts(date, time);
