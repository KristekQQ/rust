// All times are media seconds. Allow Wi-Fi jitter without repeatedly discarding video.
export function playbackDecision(current, ranges) {
  const latest = ranges[ranges.length - 1];
  if (!latest) return {seek: null, rate: 1};
  if (latest.end - current > 1.5) {
    return {seek: Math.max(latest.start, latest.end - 0.35), rate: 1};
  }
  const range = ranges.find(r => current >= r.start && current < r.end);
  if (!range) {
    const next = ranges.find(r => r.start > current);
    return {seek: next ? next.start : null, rate: 1};
  }
  const ahead = range.end - current;
  // Small speed changes build/reduce reserve without visible seek jumps.
  return {seek: null, rate: ahead < 0.2 ? 0.97 : ahead > 0.65 ? 1.03 : 1};
}
