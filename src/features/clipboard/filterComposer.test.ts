import { describe, expect, it } from "vitest";
import {
  evaluateFilterPipeline,
  FILTER_COMPOSER_MAX_OUTPUT_BYTES,
  FilterComposerEvaluationError,
  type FilterComposerStepKind,
} from "./filterComposer";

const parityCases: Array<{
  kind: FilterComposerStepKind;
  input: string;
  output: string;
}> = [
  { kind: "uppercase", input: "ClipRiva", output: "CLIPRIVA" },
  { kind: "lowercase", input: "ClipRiva", output: "clipriva" },
  { kind: "trim_whitespace", input: "  note\n", output: "note" },
  {
    kind: "normalize_whitespace",
    input: "one\n  two\tthree",
    output: "one two three",
  },
  {
    kind: "format_json",
    input: '{"b":2,"a":1}',
    output: '{\n  "a": 1,\n  "b": 2\n}',
  },
  {
    kind: "remove_blank_lines",
    input: "one\n \n two\r\n\n",
    output: "one\n two",
  },
  { kind: "deduplicate_lines", input: "b\na\nb\n", output: "b\na\n" },
  { kind: "sort_lines_asc", input: "b\nc\na", output: "a\nb\nc" },
  { kind: "sort_lines_desc", input: "b\nc\na", output: "c\nb\na" },
];

describe("evaluateFilterPipeline", () => {
  it.each(parityCases)("matches the native $kind transform contract", ({ kind, input, output }) => {
    expect(evaluateFilterPipeline(input, [{ kind }])).toBe(output);
  });

  it("applies multiple steps in their declared order", () => {
    expect(
      evaluateFilterPipeline("  beta\nalpha\nbeta  ", [
        { kind: "trim_whitespace" },
        { kind: "deduplicate_lines" },
        { kind: "sort_lines_asc" },
        { kind: "uppercase" },
      ]),
    ).toBe("ALPHA\nBETA");
  });

  it.each([0, 9])("rejects a %i-step pipeline with a finite error", (stepCount) => {
    const steps = Array.from({ length: stepCount }, () => ({
      kind: "trim_whitespace" as const,
    }));
    expect(() => evaluateFilterPipeline("private-input", steps)).toThrow(
      "A local text filter requires 1 to 8 steps.",
    );
  });

  it("rejects invalid JSON without echoing the input", () => {
    const privateInput = "not-json-private-value";
    try {
      evaluateFilterPipeline(privateInput, [{ kind: "format_json" }]);
      throw new Error("Expected the JSON step to fail.");
    } catch (error) {
      expect(error).toBeInstanceOf(FilterComposerEvaluationError);
      expect(error).toMatchObject({ code: "invalid_json", stepIndex: 0 });
      expect(String(error)).not.toContain(privateInput);
    }
  });

  it("checks every intermediate result against the UTF-8 byte limit", () => {
    const privateInput = "İ".repeat(FILTER_COMPOSER_MAX_OUTPUT_BYTES / 2);
    try {
      evaluateFilterPipeline(privateInput, [{ kind: "lowercase" }]);
      throw new Error("Expected the output limit to fail.");
    } catch (error) {
      expect(error).toMatchObject({ code: "output_too_large", stepIndex: 0 });
      expect(String(error)).not.toContain(privateInput);
    }
  });
});
