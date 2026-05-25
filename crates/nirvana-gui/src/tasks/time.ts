import { TaskSession } from "../types/types";

export const durationMs = (session: TaskSession, now: Date) => {
  const end = session.end ?? now;
  return Math.max(0, end.getTime() - session.start.getTime());
};

export const startOfDay = (date: Date) => {
  const copy = new Date(date);
  copy.setHours(0, 0, 0, 0);
  return copy;
};

export const isSameDay = (date: Date, reference: Date) =>
  startOfDay(date).getTime() === startOfDay(reference).getTime();

export const addDays = (date: Date, days: number) => {
  const copy = new Date(date);
  copy.setDate(copy.getDate() + days);
  return copy;
};

export const dayBounds = (date: Date) => {
  const start = startOfDay(date);
  return {
    start,
    end: startOfDay(addDays(start, 1)),
  };
};

export const sessionOverlapsDay = (
  session: TaskSession,
  selectedDate: Date,
  now: Date,
) => {
  const sessionEnd = session.end ?? now;
  return timestampRangeOverlapsDay(session.start, sessionEnd, selectedDate);
};

export const timestampRangeOverlapsDay = (
  start: Date,
  end: Date,
  selectedDate: Date,
) => {
  const bounds = dayBounds(selectedDate);
  return start.getTime() < bounds.end.getTime() && end.getTime() > bounds.start.getTime();
};

export const sessionStartsBeforeDay = (
  session: TaskSession,
  selectedDate: Date,
) => session.start.getTime() < dayBounds(selectedDate).start.getTime();

export const sessionEndsAfterDay = (
  session: TaskSession,
  selectedDate: Date,
  now: Date,
) => (session.end ?? now).getTime() > dayBounds(selectedDate).end.getTime();

export const sessionOverlapMs = (
  session: TaskSession,
  selectedDate: Date,
  now: Date,
) => {
  const bounds = dayBounds(selectedDate);
  const sessionEnd = session.end ?? now;
  const overlapStart = Math.max(session.start.getTime(), bounds.start.getTime());
  const overlapEnd = Math.min(sessionEnd.getTime(), bounds.end.getTime());

  return Math.max(0, overlapEnd - overlapStart);
};

export const formatDuration = (ms: number) => {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  if (totalSeconds < 60) return `${totalSeconds}s`;

  const totalMinutes = Math.floor(totalSeconds / 60);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;

  if (hours === 0) return `${minutes}m`;
  return `${hours}h ${minutes.toString().padStart(2, "0")}m`;
};

export const formatClock = (date: Date) =>
  new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);

export const formatDayLabel = (date: Date) =>
  new Intl.DateTimeFormat(undefined, {
    weekday: "short",
    month: "short",
    day: "numeric",
  }).format(date);
