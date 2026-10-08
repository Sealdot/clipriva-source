export const FILTER_COMPOSER_MAX_STEPS = 8;
export const FILTER_COMPOSER_MAX_OUTPUT_BYTES = 256 * 1024;

export const FILTER_COMPOSER_STEP_KINDS = [
  "uppercase",
  "lowercase",
  "trim_whitespace",
  "normalize_whitespace",
  "format_json",
  "remove_blank_lines",
  "deduplicate_lines",
  "sort_lines_asc",
  "sort_lines_desc",
] as const;

export type FilterComposerStepKind = (typeof FILTER_COMPOSER_STEP_KINDS)[number];

export const FILTER_COMPOSER_STEP_LABELS: Record<FilterComposerStepKind, string> = {
  uppercase: "Uppercase",
  lowercase: "Lowercase",
  trim_whitespace: "Trim whitespace",
  normalize_whitespace: "Normalize whitespace",
  format_json: "Format JSON",
  remove_blank_lines: "Remove blank lines",
  deduplicate_lines: "Deduplicate lines",
  sort_lines_asc: "Sort lines A–Z",
  sort_lines_desc: "Sort lines Z–A",
};

export interface FilterComposerStep {
  id: string;
  kind: FilterComposerStepKind;
}

export interface FilterComposerDraft {
  name: string;
  steps: FilterComposerStepKind[];
  shortcutSlot: number | null;
}

export type FilterComposerErrorCode =
  | "invalid_step_count"
  | "unsupported_step"
  | "invalid_json"
  | "output_too_large";

/** A finite, content-free error suitable for an inline local preview. */
export class FilterComposerEvaluationError extends Error {
  readonly code: FilterComposerErrorCode;
  readonly stepIndex: number | null;

  constructor(code: FilterComposerErrorCode, message: string, stepIndex: number | null = null) {
    super(message);
    this.name = "FilterComposerEvaluationError";
    this.code = code;
    this.stepIndex = stepIndex;
  }
}

/**
 * Browser-side parity evaluator for immediate previews. Saved filters must be
 * evaluated again by the native allow-list before their result is copied.
 */
export function evaluateFilterPipeline(
  input: string,
  steps: readonly Pick<FilterComposerStep, "kind">[],
): string {
  if (steps.length < 1 || steps.length > FILTER_COMPOSER_MAX_STEPS) {
    throw new FilterComposerEvaluationError(
      "invalid_step_count",
      "A local text filter requires 1 to 8 steps.",
    );
  }

  let output = input;
  steps.forEach((step, stepIndex) => {
    output = applyFilterStep(step.kind, output, stepIndex);
    if (utf8ByteLength(output) > FILTER_COMPOSER_MAX_OUTPUT_BYTES) {
      throw new FilterComposerEvaluationError(
        "output_too_large",
        "A local text filter result exceeds 256 KiB.",
        stepIndex,
      );
    }
  });
  return output;
}

function applyFilterStep(kind: FilterComposerStepKind, input: string, stepIndex: number): string {
  switch (kind) {
    case "uppercase":
      return input.toUpperCase();
    case "lowercase":
      return input.toLowerCase();
    case "trim_whitespace":
      return input.trim();
    case "normalize_whitespace":
      return input.trim().split(/\s+/u).filter(Boolean).join(" ");
    case "format_json":
      try {
        return JSON.stringify(sortJsonValue(JSON.parse(input)), null, 2);
      } catch {
        throw new FilterComposerEvaluationError(
          "invalid_json",
          "The JSON formatter accepts valid JSON only.",
          stepIndex,
        );
      }
    case "remove_blank_lines":
      return splitLines(input)
        .filter((line) => line.trim().length > 0)
        .join("\n");
    case "deduplicate_lines": {
      const seen = new Set<string>();
      return splitLines(input)
        .filter((line) => {
          if (seen.has(line)) return false;
          seen.add(line);
          return true;
        })
        .join("\n");
    }
    case "sort_lines_asc":
      return splitLines(input).sort(compareUnicodeCodePoints).join("\n");
    case "sort_lines_desc":
      return splitLines(input).sort(compareUnicodeCodePoints).reverse().join("\n");
    default:
      throw new FilterComposerEvaluationError(
        "unsupported_step",
        "This local text filter contains an unsupported step.",
        stepIndex,
      );
  }
}

function splitLines(input: string) {
  return input.split("\n").map((line) => (line.endsWith("\r") ? line.slice(0, -1) : line));
}

function compareUnicodeCodePoints(left: string, right: string) {
  const leftPoints = Array.from(left, (character) => character.codePointAt(0) ?? 0);
  const rightPoints = Array.from(right, (character) => character.codePointAt(0) ?? 0);
  const length = Math.min(leftPoints.length, rightPoints.length);
  for (let index = 0; index < length; index += 1) {
    const difference = (leftPoints[index] ?? 0) - (rightPoints[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return leftPoints.length - rightPoints.length;
}

function sortJsonValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sortJsonValue);
  if (typeof value !== "object" || value === null) return value;

  const sorted: Record<string, unknown> = Object.create(null) as Record<string, unknown>;
  for (const key of Object.keys(value).sort(compareUnicodeCodePoints)) {
    sorted[key] = sortJsonValue((value as Record<string, unknown>)[key]);
  }
  return sorted;
}

function utf8ByteLength(value: string) {
  return new TextEncoder().encode(value).byteLength;
}
