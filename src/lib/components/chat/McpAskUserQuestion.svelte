<script lang="ts">
  import { MessageCircleQuestion, ArrowUp } from "lucide-svelte";
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import type { McpQuestion } from "$lib/ipc";

  interface Props {
    questions: McpQuestion[];
    requestId: string;
    onAnswer: (requestId: string, answer: string) => Promise<void>;
  }

  let { questions, requestId, onAnswer }: Props = $props();

  let totalQ = $derived(questions.length);

  // Internal state — owned by this component since the request lifecycle
  // is shorter than the workspace; no need to survive virtualization.
  const selectedOptions = new SvelteMap<number, SvelteSet<string>>();
  const customInputs = new SvelteMap<number, string>();
  const showCustomInput = new SvelteSet<number>();
  const answers = new SvelteMap<number, string>();

  let submitted = $state(false);
  let submitting = $state(false);

  let answeredCount = $derived(answers.size);

  async function sendAnswer(text: string) {
    if (submitting || submitted) return;
    submitting = true;
    try {
      await onAnswer(requestId, text);
      submitted = true;
    } finally {
      submitting = false;
    }
  }

  function recordAnswer(qi: number, answer: string) {
    answers.set(qi, answer);
    if (totalQ === 1) {
      submitAll();
    }
  }

  function submitAll() {
    if (answers.size < totalQ) return;
    if (totalQ === 1) {
      sendAnswer(answers.get(0)!);
    } else {
      const parts: string[] = [];
      for (let i = 0; i < totalQ; i++) {
        parts.push(`${i + 1}. ${answers.get(i) ?? "(no answer)"}`);
      }
      sendAnswer(parts.join("\n"));
    }
  }

  function toggleOption(qi: number, label: string) {
    let current = selectedOptions.get(qi);
    if (!current) {
      current = new SvelteSet<string>();
      selectedOptions.set(qi, current);
    }
    if (current.has(label)) {
      current.delete(label);
    } else {
      current.add(label);
    }
  }

  function submitMultiSelect(qi: number) {
    const selected = selectedOptions.get(qi);
    if (!selected || selected.size === 0) return;
    const custom = customInputs.get(qi)?.trim();
    const parts = [...selected];
    if (custom) parts.push(custom);
    recordAnswer(qi, parts.join(", "));
  }

  function submitCustomInput(qi: number) {
    const text = customInputs.get(qi)?.trim();
    if (!text) return;
    recordAnswer(qi, text);
  }

  function submitOption(qi: number, label: string) {
    recordAnswer(qi, label);
  }
</script>

{#if submitted}
  <div class="question-card answered">
    <div class="question-header">
      <span class="question-icon"><MessageCircleQuestion size={15} strokeWidth={2} /></span>
      <span class="question-label">{totalQ === 1 ? (questions[0].header || "Question") : `${totalQ} questions`}</span>
    </div>
    <div class="question-answers-summary">
      {#each questions as q, qi (qi)}
        <div class="answer-summary-row">
          <span class="answer-summary-label">{q.header || q.question || `Q${qi + 1}`}</span>
          <span class="question-answer-pill">{answers.get(qi)}</span>
        </div>
      {/each}
    </div>
  </div>
{:else}
  {#each questions as q, qi (qi)}
    {@const isMulti = q.multiSelect === true}
    {@const answerText = answers.get(qi)}
    {@const hasAnswer = answerText != null}
    {@const selected = selectedOptions.get(qi) ?? new SvelteSet()}
    {@const customText = customInputs.get(qi) ?? ""}
    {@const showCustom = showCustomInput.has(qi)}
    <div class="question-card">
      <div class="question-header">
        <span class="question-icon"><MessageCircleQuestion size={15} strokeWidth={2} /></span>
        <span class="question-label">{q.header || "Question"}</span>
        {#if isMulti}
          <span class="question-multi-badge">Multi-select</span>
        {/if}
        {#if hasAnswer}
          <span class="question-answer-pill">{answerText}</span>
        {/if}
      </div>
      {#if q.question}
        <div class="question-text">{q.question}</div>
      {/if}
      {#if q.options && q.options.length > 0}
        <div class="question-options">
          {#each q.options as opt (opt.label)}
            {#if isMulti}
              <button
                type="button"
                class="question-option"
                class:selected={selected.has(opt.label)}
                disabled={submitting}
                onclick={() => toggleOption(qi, opt.label)}
              >
                <span class="option-check">{selected.has(opt.label) ? "◉" : "○"}</span>
                <span class="option-content">
                  <span class="option-label">{opt.label}</span>
                  {#if opt.description}
                    <span class="option-desc">{opt.description}</span>
                  {/if}
                </span>
              </button>
            {:else}
              <button
                type="button"
                class="question-option"
                class:selected-answer={hasAnswer && answerText === opt.label}
                disabled={submitting}
                onclick={() => submitOption(qi, opt.label)}
              >
                <span class="option-content">
                  <span class="option-label">{opt.label}</span>
                  {#if opt.description}
                    <span class="option-desc">{opt.description}</span>
                  {/if}
                </span>
              </button>
            {/if}
          {/each}
          {#if showCustom}
            <div class="custom-input-row">
              <input
                type="text"
                class="custom-input"
                placeholder="Type your answer…"
                value={customText}
                disabled={submitting}
                oninput={(e) => customInputs.set(qi, (e.target as HTMLInputElement).value)}
                onkeydown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); isMulti ? submitMultiSelect(qi) : submitCustomInput(qi); } }}
              />
              <button
                type="button"
                class="custom-submit-btn"
                disabled={submitting || (!customText.trim() && (!isMulti || selected.size === 0))}
                onclick={() => isMulti ? submitMultiSelect(qi) : submitCustomInput(qi)}
              >
                <ArrowUp size={14} strokeWidth={2.5} />
              </button>
            </div>
          {:else}
            <button
              type="button"
              class="question-option other-option"
              disabled={submitting}
              onclick={() => showCustomInput.add(qi)}
            >
              <span class="option-content">
                <span class="option-label">Other</span>
                <span class="option-desc">Type a custom answer</span>
              </span>
            </button>
          {/if}
          {#if isMulti && selected.size > 0}
            <button
              type="button"
              class="multi-submit-btn"
              disabled={submitting}
              onclick={() => submitMultiSelect(qi)}
            >
              Submit ({selected.size} selected)
            </button>
          {/if}
        </div>
      {/if}
    </div>
  {/each}
  {#if totalQ > 1 && answeredCount >= totalQ}
    <button
      type="button"
      class="batch-submit-btn"
      disabled={submitting}
      onclick={() => submitAll()}
    >
      <ArrowUp size={14} strokeWidth={2.5} />
      Submit all {totalQ} answers
    </button>
  {:else if totalQ > 1 && answeredCount > 0}
    <div class="batch-progress">
      {answeredCount} of {totalQ} questions answered
    </div>
  {/if}
{/if}

<style>
  .question-card {
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    border-radius: 8px;
    background: color-mix(in srgb, var(--accent) 6%, var(--bg-card));
    overflow: hidden;
  }

  .question-card.answered {
    opacity: 0.65;
    border-color: color-mix(in srgb, var(--accent) 15%, transparent);
  }

  .question-header {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.4rem 0.75rem;
    border-bottom: 1px solid color-mix(in srgb, var(--accent) 15%, transparent);
    font-size: 0.72rem;
    font-weight: 500;
    color: var(--accent);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .question-icon {
    display: flex;
    align-items: center;
    opacity: 0.8;
  }

  .question-text {
    padding: 0.55rem 0.75rem;
    font-size: 0.85rem;
    line-height: 1.55;
    color: var(--text-primary);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .question-options {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    padding: 0.4rem 0.75rem 0.6rem;
  }

  .question-multi-badge {
    margin-left: auto;
    font-size: 0.62rem;
    font-weight: 400;
    text-transform: none;
    letter-spacing: 0;
    color: var(--text-dim);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    padding: 0.1rem 0.4rem;
    border-radius: 8px;
  }

  .question-option {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.45rem 0.7rem;
    background: color-mix(in srgb, var(--accent) 4%, var(--bg-base));
    border: 1px solid color-mix(in srgb, var(--accent) 20%, transparent);
    border-radius: 6px;
    text-align: left;
    cursor: pointer;
    transition: all 0.15s ease;
    font-family: inherit;
  }

  .question-option:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 12%, var(--bg-base));
    border-color: color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .question-option.selected {
    background: color-mix(in srgb, var(--accent) 15%, var(--bg-base));
    border-color: color-mix(in srgb, var(--accent) 50%, transparent);
  }

  .question-option:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .question-option.other-option {
    border-style: dashed;
  }

  .option-check {
    flex-shrink: 0;
    font-size: 0.85rem;
    line-height: 1;
    color: var(--accent);
    margin-top: 0.1rem;
  }

  .option-content {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .option-label {
    font-size: 0.82rem;
    font-weight: 500;
    color: var(--text-bright);
  }

  .option-desc {
    font-size: 0.75rem;
    color: var(--text-dim);
    line-height: 1.4;
  }

  .custom-input-row {
    display: flex;
    gap: 0.4rem;
    align-items: center;
  }

  .custom-input {
    flex: 1;
    padding: 0.45rem 0.7rem;
    background: color-mix(in srgb, var(--accent) 4%, var(--bg-base));
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    border-radius: 6px;
    color: var(--text-bright);
    font-family: inherit;
    font-size: 0.82rem;
    outline: none;
  }

  .custom-input::placeholder {
    color: var(--text-dim);
  }

  .custom-input:focus {
    border-color: var(--accent);
  }

  .custom-submit-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    background: var(--accent);
    border: none;
    border-radius: 6px;
    color: var(--bg-base);
    cursor: pointer;
    flex-shrink: 0;
  }

  .custom-submit-btn:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .custom-submit-btn:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .multi-submit-btn {
    padding: 0.4rem 0.75rem;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
    border-radius: 6px;
    color: var(--accent);
    font-family: inherit;
    font-size: 0.8rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .multi-submit-btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    border-color: var(--accent);
  }

  .multi-submit-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .question-option.selected-answer {
    background: color-mix(in srgb, var(--accent) 15%, var(--bg-base));
    border-color: color-mix(in srgb, var(--accent) 50%, transparent);
  }

  .question-answer-pill {
    margin-left: auto;
    font-size: 0.68rem;
    font-weight: 400;
    text-transform: none;
    letter-spacing: 0;
    color: var(--text-bright);
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    padding: 0.12rem 0.5rem;
    border-radius: 8px;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .batch-submit-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    width: 100%;
    padding: 0.55rem 0.75rem;
    margin-top: 0.3rem;
    background: color-mix(in srgb, var(--accent) 15%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 50%, transparent);
    border-radius: 8px;
    color: var(--accent);
    font-family: inherit;
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .batch-submit-btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 25%, transparent);
    border-color: var(--accent);
  }

  .batch-submit-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .question-answers-summary {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    padding: 0.35rem 0.75rem 0.45rem;
  }

  .answer-summary-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.78rem;
  }

  .answer-summary-label {
    color: var(--text-dim);
    flex-shrink: 0;
  }

  .batch-progress {
    text-align: center;
    font-size: 0.72rem;
    color: var(--text-dim);
    padding: 0.4rem 0 0.1rem;
  }
</style>
