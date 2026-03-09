export type RegimeLabel =
    | 'TREND_UP'
    | 'TREND_DOWN'
    | 'RANGE'
    | 'VOL_COMPRESSION'
    | 'VOL_EXPANSION'
    | 'PANIC'
    | 'ILLIQUID'
    | 'HIGH_SPREAD_NO_TRADE';

export interface FactorInput {
    name: string;
    series: number[];
    returnSeries: number[];
    weight?: number;
    rollingWindow?: number;
    currentValue?: number;
    directionHint?: -1 | 1;
    enabled?: boolean;
}

export interface RegimeFilterConfig {
    enabled?: boolean;
    currentRegime?: RegimeLabel | string;
    allowedRegimes?: Array<RegimeLabel | string>;
}

export interface VolatilityFilterConfig {
    enabled?: boolean;
    historicalVolatilitySeries?: number[];
    currentVolatility?: number;
    minZScore?: number;
    maxZScore?: number;
}

export type WeightMode =
    | 'equal'
    | 'manual'
    | 'correlation'
    | 'ranked-correlation';

export interface SignalBuildConfig {
    rollingWindow?: number;
    minHistory?: number;
    minAbsCorrelation?: number;
    rankDecay?: number;
    weightMode?: WeightMode;
    winsorizeZScoreAt?: number;
    meanReturn?: number;
    stdReturn?: number;
    regimeFilter?: RegimeFilterConfig;
    volatilityFilter?: VolatilityFilterConfig;
}

export interface FactorContribution {
    name: string;
    correlation: number;
    sign: number;
    zscore: number;
    rank: number | null;
    rawWeight: number;
    normalizedWeight: number;
    contribution: number;
    passed: boolean;
    rejectReason?: string;
}

export interface SignalResult {
    signal: number;
    predictedReturn: number;
    meanReturn: number;
    stdReturn: number;
    suppressed: boolean;
    suppressionReasons: string[];
    regimePassed: boolean;
    volatilityPassed: boolean;
    factorsUsed: number;
    totalFactors: number;
    contributions: FactorContribution[];
}

const DEFAULT_CONFIG: Required<
    Omit<SignalBuildConfig, 'meanReturn' | 'stdReturn' | 'regimeFilter' | 'volatilityFilter'>
> = {
    rollingWindow: 120,
    minHistory: 30,
    minAbsCorrelation: 0.03,
    rankDecay: 0.85,
    weightMode: 'equal',
    winsorizeZScoreAt: 4,
};

const EPSILON = 1e-12;

function isFiniteNumber(value: unknown): value is number {
    return typeof value === 'number' && Number.isFinite(value);
}

function sanitizeSeries(values: number[]): number[] {
    return values.filter((value) => Number.isFinite(value));
}

function sanitizeAlignedPair(seriesA: number[], seriesB: number[]): [number[], number[]] {
    const len = Math.min(seriesA.length, seriesB.length);
    const aOffset = seriesA.length - len;
    const bOffset = seriesB.length - len;
    const alignedA: number[] = [];
    const alignedB: number[] = [];

    for (let i = 0; i < len; i += 1) {
        const a = seriesA[aOffset + i];
        const b = seriesB[bOffset + i];
        if (Number.isFinite(a) && Number.isFinite(b)) {
            alignedA.push(a);
            alignedB.push(b);
        }
    }

    return [alignedA, alignedB];
}

function tail(values: number[], n: number): number[] {
    if (n <= 0) return [];
    if (values.length <= n) return values;
    return values.slice(values.length - n);
}

function clamp(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value));
}

function pearsonCorrelation(seriesA: number[], seriesB: number[]): number {
    const len = Math.min(seriesA.length, seriesB.length);
    if (len < 2) return 0;

    const a = seriesA.slice(seriesA.length - len);
    const b = seriesB.slice(seriesB.length - len);
    const meanA = calculateMean(a);
    const meanB = calculateMean(b);

    let covariance = 0;
    let varA = 0;
    let varB = 0;

    for (let i = 0; i < len; i += 1) {
        const da = a[i] - meanA;
        const db = b[i] - meanB;
        covariance += da * db;
        varA += da * da;
        varB += db * db;
    }

    if (varA <= EPSILON || varB <= EPSILON) return 0;
    return covariance / Math.sqrt(varA * varB);
}

function calculateRollingStats(series: number[], window: number): { mean: number; std: number } {
    const segment = tail(series, window);
    return {
        mean: calculateMean(segment),
        std: calculateStd(segment),
    };
}

function evaluateRegimeFilter(config?: RegimeFilterConfig): {
    passed: boolean;
    reason?: string;
} {
    if (!config?.enabled) {
        return { passed: true };
    }

    const current = config.currentRegime?.toString().trim();
    const allowed = (config.allowedRegimes ?? []).map((regime) => regime.toString());

    if (!current) {
        return { passed: false, reason: 'regime_filter_missing_current' };
    }

    if (allowed.length === 0) {
        return { passed: false, reason: 'regime_filter_empty_allowed_set' };
    }

    if (!allowed.includes(current)) {
        return { passed: false, reason: `regime_not_allowed:${current}` };
    }

    return { passed: true };
}

function evaluateVolatilityFilter(config?: VolatilityFilterConfig): {
    passed: boolean;
    zscore: number;
    reason?: string;
} {
    if (!config?.enabled) {
        return { passed: true, zscore: 0 };
    }

    const history = sanitizeSeries(config.historicalVolatilitySeries ?? []);
    if (history.length < 2) {
        return { passed: false, zscore: 0, reason: 'vol_filter_insufficient_history' };
    }

    const currentVol = isFiniteNumber(config.currentVolatility)
        ? config.currentVolatility
        : history[history.length - 1];

    if (!isFiniteNumber(currentVol)) {
        return { passed: false, zscore: 0, reason: 'vol_filter_missing_current' };
    }

    const mean = calculateMean(history);
    const std = calculateStd(history);
    const zscore = calculateZScore(currentVol, mean, std);
    const minZ = isFiniteNumber(config.minZScore) ? config.minZScore : -Infinity;
    const maxZ = isFiniteNumber(config.maxZScore) ? config.maxZScore : Infinity;

    if (zscore < minZ || zscore > maxZ) {
        return {
            passed: false,
            zscore,
            reason: `vol_z_out_of_range:${zscore.toFixed(4)}`,
        };
    }

    return { passed: true, zscore };
}

export function calculateMean(values: number[]): number {
    const sanitized = sanitizeSeries(values);
    if (sanitized.length === 0) {
        throw new Error('calculateMean requires at least one finite number.');
    }

    const total = sanitized.reduce((sum, value) => sum + value, 0);
    return total / sanitized.length;
}

export function calculateStd(values: number[]): number {
    const sanitized = sanitizeSeries(values);
    if (sanitized.length === 0) {
        throw new Error('calculateStd requires at least one finite number.');
    }

    if (sanitized.length === 1) {
        return 0;
    }

    const mean = calculateMean(sanitized);
    const variance =
        sanitized.reduce((sum, value) => {
            const delta = value - mean;
            return sum + delta * delta;
        }, 0) / sanitized.length;

    return Math.sqrt(Math.max(variance, 0));
}

export function calculateZScore(value: number, mean: number, std: number): number {
    if (!Number.isFinite(value) || !Number.isFinite(mean) || !Number.isFinite(std)) {
        throw new Error('calculateZScore requires finite value, mean, and std.');
    }

    if (Math.abs(std) <= EPSILON) {
        return 0;
    }

    return (value - mean) / std;
}

export function normalizeFactorSeries(series: number[]): number[] {
    const sanitized = sanitizeSeries(series);
    if (sanitized.length === 0) {
        return [];
    }

    const mean = calculateMean(sanitized);
    const std = calculateStd(sanitized);
    return sanitized.map((value) => calculateZScore(value, mean, std));
}

export function detectFactorCorrelationSign(
    factorSeries: number[],
    returnSeries: number[],
): number {
    const [factor, returns] = sanitizeAlignedPair(factorSeries, returnSeries);
    const correlation = pearsonCorrelation(factor, returns);

    if (Math.abs(correlation) <= EPSILON) {
        return 0;
    }

    return correlation > 0 ? 1 : -1;
}

export function buildZScoreSignal(factors: FactorInput[]): SignalResult;
export function buildZScoreSignal(
    factors: FactorInput[],
    config: SignalBuildConfig = {},
): SignalResult {
    if (factors.length === 0) {
        return {
            signal: 0,
            predictedReturn: 0,
            meanReturn: 0,
            stdReturn: 0,
            suppressed: true,
            suppressionReasons: ['no_factors'],
            regimePassed: true,
            volatilityPassed: true,
            factorsUsed: 0,
            totalFactors: 0,
            contributions: [],
        };
    }

    const cfg: SignalBuildConfig = {
        ...DEFAULT_CONFIG,
        ...config,
    };

    const suppressionReasons: string[] = [];
    const regimeGate = evaluateRegimeFilter(cfg.regimeFilter);
    const volGate = evaluateVolatilityFilter(cfg.volatilityFilter);

    if (!regimeGate.passed && regimeGate.reason) {
        suppressionReasons.push(regimeGate.reason);
    }
    if (!volGate.passed && volGate.reason) {
        suppressionReasons.push(volGate.reason);
    }

    const prepared = factors.map((factor, factorIndex) => {
        const factorSeries = sanitizeSeries(factor.series);
        const returnSeries = sanitizeSeries(factor.returnSeries);
        const rollingWindow = Math.max(
            2,
            Math.floor(factor.rollingWindow ?? cfg.rollingWindow ?? DEFAULT_CONFIG.rollingWindow),
        );

        if (factor.enabled === false) {
            return {
                factorIndex,
                factor,
                passed: false,
                rejectReason: 'factor_disabled',
                correlation: 0,
                sign: 0,
                zscore: 0,
                rawWeight: 0,
                contribution: 0,
            };
        }

        if (factorSeries.length < (cfg.minHistory ?? DEFAULT_CONFIG.minHistory)) {
            return {
                factorIndex,
                factor,
                passed: false,
                rejectReason: 'insufficient_factor_history',
                correlation: 0,
                sign: 0,
                zscore: 0,
                rawWeight: 0,
                contribution: 0,
            };
        }

        if (returnSeries.length < (cfg.minHistory ?? DEFAULT_CONFIG.minHistory)) {
            return {
                factorIndex,
                factor,
                passed: false,
                rejectReason: 'insufficient_return_history',
                correlation: 0,
                sign: 0,
                zscore: 0,
                rawWeight: 0,
                contribution: 0,
            };
        }

        const alignedLength = Math.min(factorSeries.length, returnSeries.length, rollingWindow);
        if (alignedLength < 2) {
            return {
                factorIndex,
                factor,
                passed: false,
                rejectReason: 'insufficient_overlap',
                correlation: 0,
                sign: 0,
                zscore: 0,
                rawWeight: 0,
                contribution: 0,
            };
        }

        const factorTail = tail(factorSeries, alignedLength);
        const returnTail = tail(returnSeries, alignedLength);
        const [alignedFactorTail, alignedReturnTail] = sanitizeAlignedPair(factorTail, returnTail);
        if (alignedFactorTail.length < 2) {
            return {
                factorIndex,
                factor,
                passed: false,
                rejectReason: 'insufficient_finite_overlap',
                correlation: 0,
                sign: 0,
                zscore: 0,
                rawWeight: 0,
                contribution: 0,
            };
        }

        const correlation = pearsonCorrelation(alignedFactorTail, alignedReturnTail);
        const sign =
            factor.directionHint ?? detectFactorCorrelationSign(alignedFactorTail, alignedReturnTail);

        if (Math.abs(correlation) < (cfg.minAbsCorrelation ?? DEFAULT_CONFIG.minAbsCorrelation)) {
            return {
                factorIndex,
                factor,
                passed: false,
                rejectReason: 'low_predictive_correlation',
                correlation,
                sign,
                zscore: 0,
                rawWeight: 0,
                contribution: 0,
            };
        }

        const currentValue = isFiniteNumber(factor.currentValue)
            ? factor.currentValue
            : factorSeries[factorSeries.length - 1];

        if (!isFiniteNumber(currentValue)) {
            return {
                factorIndex,
                factor,
                passed: false,
                rejectReason: 'missing_current_factor_value',
                correlation,
                sign,
                zscore: 0,
                rawWeight: 0,
                contribution: 0,
            };
        }

        const { mean, std } = calculateRollingStats(factorSeries, rollingWindow);
        const rawZScore = calculateZScore(currentValue, mean, std);
        const boundedZScore = clamp(
            rawZScore,
            -(cfg.winsorizeZScoreAt ?? DEFAULT_CONFIG.winsorizeZScoreAt),
            cfg.winsorizeZScoreAt ?? DEFAULT_CONFIG.winsorizeZScoreAt,
        );

        return {
            factorIndex,
            factor,
            passed: true,
            rejectReason: undefined,
            correlation,
            sign,
            zscore: boundedZScore,
            rawWeight: 0,
            contribution: 0,
        };
    });

    const ranked = prepared
        .filter((row) => row.passed)
        .sort((a, b) => Math.abs(b.correlation) - Math.abs(a.correlation));

    const rankDecay = cfg.rankDecay ?? DEFAULT_CONFIG.rankDecay;
    ranked.forEach((row, index) => {
        const absCorr = Math.abs(row.correlation);
        const manualWeight = Math.max(0, row.factor.weight ?? 1);
        const rankWeight = Math.pow(rankDecay, index);
        const mode = cfg.weightMode ?? DEFAULT_CONFIG.weightMode;

        switch (mode) {
            case 'equal':
                row.rawWeight = 1;
                break;
            case 'manual':
                row.rawWeight = manualWeight;
                break;
            case 'correlation':
                row.rawWeight = manualWeight * absCorr;
                break;
            case 'ranked-correlation':
            default:
                row.rawWeight = manualWeight * absCorr * rankWeight;
                break;
        }

        row.contribution = row.sign * row.zscore;
    });

    const totalWeight = ranked.reduce((sum, row) => sum + row.rawWeight, 0);
    if (ranked.length > 0 && totalWeight <= EPSILON) {
        suppressionReasons.push('all_factor_weights_zero');
    }
    if (ranked.length === 0) {
        suppressionReasons.push('no_valid_factors_after_filters');
    }

    ranked.forEach((row) => {
        row.rawWeight = Number.isFinite(row.rawWeight) ? row.rawWeight : 0;
    });

    const safeTotalWeight = ranked.reduce((sum, row) => sum + row.rawWeight, 0);
    const normalizedWeightDenominator = safeTotalWeight > EPSILON ? safeTotalWeight : 1;

    const weightedSignal = ranked.reduce((sum, row) => {
        const normalizedWeight = row.rawWeight / normalizedWeightDenominator;
        return sum + normalizedWeight * row.contribution;
    }, 0);

    const referenceReturns = ranked.length > 0
        ? tail(
            sanitizeSeries(ranked[0].factor.returnSeries),
            Math.max(
                cfg.minHistory ?? DEFAULT_CONFIG.minHistory,
                ranked[0].factor.rollingWindow ?? cfg.rollingWindow ?? DEFAULT_CONFIG.rollingWindow,
            ),
        )
        : [];

    const meanReturn = isFiniteNumber(cfg.meanReturn)
        ? cfg.meanReturn
        : referenceReturns.length > 0
            ? calculateMean(referenceReturns)
            : 0;
    const stdReturn = isFiniteNumber(cfg.stdReturn)
        ? Math.max(0, cfg.stdReturn)
        : referenceReturns.length > 1
            ? calculateStd(referenceReturns)
            : 0;

    const gatedOut = !regimeGate.passed || !volGate.passed;
    const modelInvalid = ranked.length === 0 || safeTotalWeight <= EPSILON;
    const suppressed = gatedOut || modelInvalid;
    const signal = suppressed ? 0 : weightedSignal;

    const contributionLookup = new Map<number, typeof ranked[number]>();
    const rankLookup = new Map<number, number>();
    ranked.forEach((row) => {
        contributionLookup.set(row.factorIndex, row);
    });
    ranked.forEach((row, index) => {
        rankLookup.set(row.factorIndex, index + 1);
    });

    const contributions: FactorContribution[] = prepared.map((row) => {
        const rankedRow = contributionLookup.get(row.factorIndex);
        if (!rankedRow) {
            return {
                name: row.factor.name,
                correlation: row.correlation,
                sign: row.sign,
                zscore: row.zscore,
                rank: null,
                rawWeight: 0,
                normalizedWeight: 0,
                contribution: 0,
                passed: false,
                rejectReason: row.rejectReason ?? 'filtered_out',
            };
        }

        const rank = rankLookup.get(row.factorIndex) ?? null;
        const normalizedWeight = rankedRow.rawWeight / normalizedWeightDenominator;

        return {
            name: row.factor.name,
            correlation: rankedRow.correlation,
            sign: rankedRow.sign,
            zscore: rankedRow.zscore,
            rank,
            rawWeight: rankedRow.rawWeight,
            normalizedWeight,
            contribution: normalizedWeight * rankedRow.contribution,
            passed: true,
        };
    });

    return {
        signal,
        predictedReturn: meanReturn + stdReturn * signal,
        meanReturn,
        stdReturn,
        suppressed,
        suppressionReasons,
        regimePassed: regimeGate.passed,
        volatilityPassed: volGate.passed,
        factorsUsed: ranked.length,
        totalFactors: factors.length,
        contributions,
    };
}
