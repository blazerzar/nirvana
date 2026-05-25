import { computed, nextTick, ref } from "vue";
import { addDays, formatClock } from "../tasks/time";
import {
  applyDateAndTimeParts,
  formatDateInput,
  formatDurationInput,
  formatTimeInput,
  formatTimeParts,
  parseDateInput,
  parseDurationInput,
  parseTimeParts,
  wrapTimePart,
} from "./dateTimeInputs";

type SessionRangeEditorOptions = {
  allowRunningStop: boolean;
  clearSubmitError: () => void;
  now: () => Date;
};

const dayMs = 24 * 60 * 60 * 1000;

export const localDayOffset = (startDate: Date, stopDate: Date) => {
  const startDay = new Date(startDate);
  const stopDay = new Date(stopDate);
  startDay.setHours(0, 0, 0, 0);
  stopDay.setHours(0, 0, 0, 0);
  return Math.round((stopDay.getTime() - startDay.getTime()) / dayMs);
};

export const stopDayLabelForOffset = (offset: number) => {
  if (offset === 0) return "same day";
  if (offset === 1) return "next day";
  if (offset > 1) return `+${offset} days`;
  if (offset === -1) return "previous day";
  return `${offset} days`;
};

export const formatShortDate = (date: Date) =>
  new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
  }).format(date);

export const formatCompletedRangePreview = (
  start: Date,
  stop: Date,
  duration: string,
  stopDayOffset: number,
) => {
  const range =
    stopDayOffset === 0
      ? `${formatShortDate(start)}, ${formatClock(start)}-${formatClock(stop)}`
      : `${formatShortDate(start)}, ${formatClock(start)}-${formatShortDate(stop)}, ${formatClock(stop)}`;

  return `${range} · ${duration}`;
};

export const formatRunningRangePreview = (start: Date) =>
  `since ${formatShortDate(start)}, ${formatClock(start)} · running`;

export const useSessionRangeEditor = ({
  allowRunningStop,
  clearSubmitError,
  now,
}: SessionRangeEditorOptions) => {
  const startDateInput = ref("");
  const stopDateInput = ref("");
  const start = ref("");
  const stop = ref("");
  const durationInput = ref("");
  const advancedStopDateVisible = ref(false);

  const parsedStartDate = computed(() => parseDateInput(startDateInput.value));
  const parsedStopDate = computed(() => parseDateInput(stopDateInput.value));

  const parsedStart = computed(() => {
    const time = parseTimeParts(start.value);
    if (!parsedStartDate.value || !time) return null;

    return applyDateAndTimeParts(parsedStartDate.value, time);
  });

  const parsedStop = computed(() => {
    const time = stop.value ? parseTimeParts(stop.value) : null;
    if (!parsedStopDate.value || !time) return null;

    return applyDateAndTimeParts(parsedStopDate.value, time);
  });

  const parsedDurationMs = computed(() => parseDurationInput(durationInput.value));
  const stopDayOffset = computed(() => {
    if (!parsedStartDate.value || !parsedStopDate.value) return 0;
    return localDayOffset(parsedStartDate.value, parsedStopDate.value);
  });
  const stopDayLabel = computed(() => stopDayLabelForOffset(stopDayOffset.value));

  const hasStopInput = () => Boolean(stopDateInput.value || stop.value);

  const syncDurationFromTimes = () => {
    if (!parsedStart.value) return;
    const end = parsedStop.value ?? (allowRunningStop ? now() : null);
    if (!end) return;

    durationInput.value = formatDurationInput(
      end.getTime() - parsedStart.value.getTime(),
    );
  };

  const preserveStopDateFromTime = () => {
    if (!parsedStart.value || !parsedStartDate.value || !stop.value.trim()) return;

    const time = parseTimeParts(stop.value);
    if (!time) return;

    const offset = stopDateInput.value ? Math.max(0, stopDayOffset.value) : 0;
    let nextStop = applyDateAndTimeParts(addDays(parsedStartDate.value, offset), time);

    if (nextStop.getTime() <= parsedStart.value.getTime()) {
      nextStop = addDays(nextStop, 1);
    }

    stopDateInput.value = formatDateInput(nextStop);
  };

  const applyDurationToStop = () => {
    if (!parsedStart.value || !parsedDurationMs.value) return;

    const nextStop = new Date(parsedStart.value.getTime() + parsedDurationMs.value);
    stopDateInput.value = formatDateInput(nextStop);
    stop.value = formatTimeInput(nextStop);
  };

  const handleStartDateInput = () => {
    clearSubmitError();
    if (!parsedStartDate.value) return;

    if (!allowRunningStop || hasStopInput()) {
      applyDurationToStop();
    } else {
      syncDurationFromTimes();
    }
  };

  const handleStartTimeInput = () => {
    clearSubmitError();
    if (!parseTimeParts(start.value)) return;

    if (!allowRunningStop || hasStopInput()) {
      applyDurationToStop();
    } else {
      syncDurationFromTimes();
    }
  };

  const handleStopTimeInput = () => {
    clearSubmitError();
    if (!parseTimeParts(stop.value)) return;

    preserveStopDateFromTime();
    syncDurationFromTimes();
  };

  const handleStopDateInput = () => {
    clearSubmitError();
    if (!parsedStopDate.value) return;

    syncDurationFromTimes();
  };

  const handleDurationInput = () => {
    clearSubmitError();
    applyDurationToStop();
  };

  const normalizeStartDate = () => {
    const date = parsedStartDate.value;
    if (!date) return null;

    startDateInput.value = formatDateInput(date);
    handleStartDateInput();
    return date;
  };

  const normalizeStopDate = () => {
    const date = parsedStopDate.value;
    if (!date) return;

    stopDateInput.value = formatDateInput(date);
    handleStopDateInput();
  };

  const normalizeStartTime = () => {
    const time = parseTimeParts(start.value);
    if (!time) return;

    start.value = formatTimeParts(time.hours, time.minutes);
    handleStartTimeInput();
  };

  const normalizeStopTime = () => {
    if (!stop.value.trim()) return;

    const time = parseTimeParts(stop.value);
    if (!time) return;

    stop.value = formatTimeParts(time.hours, time.minutes);
    handleStopTimeInput();
  };

  const normalizeDuration = () => {
    if (!parsedDurationMs.value) return;

    durationInput.value = formatDurationInput(parsedDurationMs.value);
    handleDurationInput();
  };

  const handleDurationKeydown = (event: KeyboardEvent) => {
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;

    event.preventDefault();

    const direction = event.key === "ArrowUp" ? 1 : -1;
    const currentMs = parsedDurationMs.value ?? 30 * 60 * 1000;
    const stepMs = 5 * 60 * 1000;
    const nextMs = Math.max(60 * 1000, currentMs + direction * stepMs);

    durationInput.value = formatDurationInput(nextMs);
    handleDurationInput();
  };

  const handleTimeKeydown = async (
    event: KeyboardEvent,
    value: typeof start,
    handleInput: () => void,
  ) => {
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;

    event.preventDefault();

    const input = event.currentTarget as HTMLInputElement;
    const direction = event.key === "ArrowUp" ? 1 : -1;
    const fallback = value === stop && !value.value ? start.value : value.value;
    const time = parseTimeParts(value.value) ?? parseTimeParts(fallback);
    if (!time) return;

    const separatorIndex = value.value.indexOf(":");
    const editingMinutes =
      separatorIndex !== -1 &&
      input.selectionStart !== null &&
      input.selectionStart > separatorIndex;
    const nextHours = editingMinutes
      ? time.hours
      : wrapTimePart(time.hours + direction, 24);
    const nextMinutes = editingMinutes
      ? wrapTimePart(time.minutes + direction, 60)
      : time.minutes;

    value.value = formatTimeParts(nextHours, nextMinutes);
    await nextTick();
    handleInput();

    if (editingMinutes) {
      input.setSelectionRange(3, 5);
    } else {
      input.setSelectionRange(0, 2);
    }
  };

  const handleStartKeydown = (event: KeyboardEvent) =>
    handleTimeKeydown(event, start, handleStartTimeInput);

  const handleStopKeydown = (event: KeyboardEvent) =>
    handleTimeKeydown(event, stop, handleStopTimeInput);

  const setRangeInputs = (range: {
    start: Date | null;
    stop: Date | null;
    durationMs: number | null;
  }) => {
    startDateInput.value = range.start ? formatDateInput(range.start) : "";
    start.value = range.start ? formatTimeInput(range.start) : "";
    stopDateInput.value = range.stop ? formatDateInput(range.stop) : "";
    stop.value = range.stop ? formatTimeInput(range.stop) : "";
    durationInput.value =
      range.durationMs !== null ? formatDurationInput(range.durationMs) : "";
    advancedStopDateVisible.value = stopDayOffset.value !== 0;
  };

  const toggleAdvancedStopDate = () => {
    advancedStopDateVisible.value = !advancedStopDateVisible.value;
  };

  return {
    advancedStopDateVisible,
    applyDurationToStop,
    durationInput,
    handleDurationInput,
    handleDurationKeydown,
    handleStartDateInput,
    handleStartKeydown,
    handleStartTimeInput,
    handleStopDateInput,
    handleStopKeydown,
    handleStopTimeInput,
    normalizeDuration,
    normalizeStartDate,
    normalizeStartTime,
    normalizeStopDate,
    normalizeStopTime,
    parsedDurationMs,
    parsedStart,
    parsedStartDate,
    parsedStop,
    parsedStopDate,
    setRangeInputs,
    start,
    startDateInput,
    stop,
    stopDateInput,
    stopDayLabel,
    stopDayOffset,
    toggleAdvancedStopDate,
  };
};
