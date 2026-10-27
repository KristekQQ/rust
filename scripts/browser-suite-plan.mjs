// Partition complete test cases, after backend compatibility filtering.
export function parseShard(value) {
    const match = typeof value === 'string' && /^([1-9]\d*)\/([1-9]\d*)$/.exec(value);
    if (!match) throw new Error('--shard requires index/count, starting at 1');
    const index = Number(match[1]), count = Number(match[2]);
    if (!Number.isSafeInteger(index) || !Number.isSafeInteger(count) || index > count) {
        throw new Error('--shard index must not exceed its positive integer count');
    }
    return {index, count};
}

export function partitionCases(cases, shard = null) {
    const selected = shard ? cases.filter((_, index) => index % shard.count === shard.index - 1) : cases;
    if (!selected.length) throw new Error('No compatible browser cases selected for this shard');
    return selected;
}

export function suiteTimeout(suite, {software, timeout, timeoutExplicit}) {
    return software && !timeoutExplicit ? Math.max(timeout, suite.softwareTimeoutMs || 0) : timeout;
}
