<script lang="ts">
  import RiCheckLine from "remixicon-svelte/icons/check-line";
  import RiComputerLine from "remixicon-svelte/icons/computer-line";
  import RiKeyboardLine from "remixicon-svelte/icons/keyboard-line";
  import { onMount } from "svelte";

  import * as Alert from "$lib/components/ui/alert/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import TypingPractice from "../components/TypingPractice.svelte";
  import {
    detectInstructionPlatform,
    typingLessons,
    typingTracks,
    type InstructionPlatform,
    type TypingLanguage,
  } from "../lib/typing-lessons";

  const platformPreferenceKey = "meiki-typing-platform";
  const languagePreferenceKey = "meiki-typing-language";
  const completionPreferenceKey = "meiki-typing-completed";

  let selectedLanguage = $state<TypingLanguage>("korean");
  let selectedPlatform = $state<InstructionPlatform | null>(null);
  let detectedPlatform = $state<InstructionPlatform | null>(null);
  let completedLessonIds = $state<string[]>([]);
  let practiceStarted = $state(false);
  let selectedLessonId = $state(typingLessons[0].id);
  let selectedLanguageLessons = $derived(
    typingLessons.filter((lesson) => lesson.language === selectedLanguage),
  );
  let selectedLesson = $derived(
    selectedLanguageLessons.find((lesson) => lesson.id === selectedLessonId) ??
      selectedLanguageLessons[0] ??
      typingLessons[0],
  );

  onMount(() => {
    const savedLanguage = localStorage.getItem(languagePreferenceKey);
    if (isTypingLanguage(savedLanguage)) {
      selectedLanguage = savedLanguage;
      selectedLessonId = firstLessonId(savedLanguage);
    }

    detectedPlatform = detectInstructionPlatform(navigator);
    const savedPlatform = localStorage.getItem(platformPreferenceKey);
    if (isInstructionPlatform(savedPlatform)) {
      selectedPlatform = savedPlatform;
    } else if (detectedPlatform) {
      selectedPlatform = detectedPlatform;
      localStorage.setItem(platformPreferenceKey, detectedPlatform);
    }

    const savedCompletion = localStorage.getItem(completionPreferenceKey);
    if (savedCompletion) {
      try {
        const values: unknown = JSON.parse(savedCompletion);
        if (Array.isArray(values)) {
          const lessonIds = new Set(typingLessons.map((lesson) => lesson.id));
          completedLessonIds = values.filter(
            (value): value is string =>
              typeof value === "string" && lessonIds.has(value),
          );
        }
      } catch {
        localStorage.removeItem(completionPreferenceKey);
      }
    }
  });

  function isTypingLanguage(value: string | null): value is TypingLanguage {
    return typingTracks.some((track) => track.language === value);
  }

  function isInstructionPlatform(
    value: string | null,
  ): value is InstructionPlatform {
    return value === "windows" || value === "macos";
  }

  function chooseLanguage(language: TypingLanguage): void {
    selectedLanguage = language;
    selectedLessonId = firstLessonId(language);
    practiceStarted = false;
    localStorage.setItem(languagePreferenceKey, language);
  }

  function firstLessonId(language: TypingLanguage): string {
    return (
      typingLessons.find((lesson) => lesson.language === language)?.id ??
      typingLessons[0].id
    );
  }

  function trackIsCompleted(language: TypingLanguage): boolean {
    const lessonIds = typingLessons
      .filter((lesson) => lesson.language === language)
      .map((lesson) => lesson.id);
    return (
      lessonIds.length > 0 &&
      lessonIds.every((lessonId) => completedLessonIds.includes(lessonId))
    );
  }

  function choosePlatform(platform: InstructionPlatform): void {
    selectedPlatform = platform;
    localStorage.setItem(platformPreferenceKey, platform);
  }

  function completeLesson(lessonId: string): void {
    if (completedLessonIds.includes(lessonId)) return;
    completedLessonIds = [...completedLessonIds, lessonId];
    localStorage.setItem(
      completionPreferenceKey,
      JSON.stringify(completedLessonIds),
    );
  }

  function nextLesson(): void {
    const currentIndex = selectedLanguageLessons.findIndex(
      (lesson) => lesson.id === selectedLesson.id,
    );
    const next =
      selectedLanguageLessons[
        (currentIndex + 1) % selectedLanguageLessons.length
      ];
    selectedLessonId = next.id;
    practiceStarted = true;
  }
</script>

<section class="screen typing-screen" aria-labelledby="typing-title">
  <header class="screen-header">
    <div>
      <span class="eyebrow">Local practice</span>
      <h1 id="typing-title" class="screen-title">Typing</h1>
      <p class="screen-description">
        Learn physical positions, then practice the text your input method
        commits.
      </p>
    </div>
    <Button data-primary-action onclick={() => (practiceStarted = true)}>
      <RiKeyboardLine data-icon="inline-start" aria-hidden="true" />
      Start practice
    </Button>
  </header>

  <div class="typing-settings">
    <fieldset class="choice-fieldset">
      <legend>Language</legend>
      <div class="language-choices">
        {#each typingTracks as track (track.language)}
          <Button
            class="h-auto min-h-13 justify-between whitespace-normal text-left"
            variant={selectedLanguage === track.language
              ? "secondary"
              : "outline"}
            aria-label={track.selectionLabel}
            aria-pressed={selectedLanguage === track.language}
            onclick={() => chooseLanguage(track.language)}
          >
            <span>{track.selectionLabel}</span>
            {#if trackIsCompleted(track.language)}
              <Badge variant="secondary">
                <RiCheckLine aria-hidden="true" />
                Completed
              </Badge>
            {/if}
          </Button>
        {/each}
      </div>
    </fieldset>

    <fieldset class="choice-fieldset platform-fieldset">
      <legend>Instructions</legend>
      <div
        class="platform-choices"
        role="group"
        aria-label="Instruction platform"
      >
        <Button
          variant={selectedPlatform === "windows" ? "secondary" : "outline"}
          aria-pressed={selectedPlatform === "windows"}
          onclick={() => choosePlatform("windows")}
        >
          <RiComputerLine data-icon="inline-start" aria-hidden="true" />
          Windows
        </Button>
        <Button
          variant={selectedPlatform === "macos" ? "secondary" : "outline"}
          aria-pressed={selectedPlatform === "macos"}
          onclick={() => choosePlatform("macos")}
        >
          <RiComputerLine data-icon="inline-start" aria-hidden="true" />
          macOS
        </Button>
      </div>
      {#if detectedPlatform}
        <p class="field-description">
          {detectedPlatform === "macos" ? "macOS" : "Windows"} was detected. You can
          override it at any time.
        </p>
      {/if}
    </fieldset>
  </div>

  {#if !detectedPlatform}
    <Alert.Root data-testid="typing-linux-guidance">
      <Alert.Title>Input-source setup varies on Linux</Alert.Title>
      <Alert.Description>
        Configure the language input source through your desktop environment.
        All exercises remain available; choose Windows or macOS only when that
        reference is useful for your setup.
      </Alert.Description>
    </Alert.Root>
  {/if}

  <Card.Root>
    <Card.Header>
      <Card.Title>Setup reference</Card.Title>
      <Card.Description>
        {#if selectedPlatform}
          {selectedLesson.instructions[selectedPlatform]}
        {:else}
          Configure an input source that can commit the target text. The exact
          steps depend on your desktop environment.
        {/if}
      </Card.Description>
    </Card.Header>
  </Card.Root>

  {#if practiceStarted}
    {#key `${selectedLesson.id}:${selectedPlatform ?? "variable"}`}
      <TypingPractice
        lesson={selectedLesson}
        platform={selectedPlatform}
        completed={completedLessonIds.includes(selectedLesson.id)}
        onComplete={completeLesson}
        onNext={nextLesson}
      />
    {/key}
  {:else}
    <Card.Root>
      <Card.Header>
        <Card.Title>{selectedLesson.title}</Card.Title>
        <Card.Description>{selectedLesson.hint}</Card.Description>
      </Card.Header>
      <Card.Content>
        <p class="m-0 text-sm text-muted-foreground">
          This foundation exercise stays on your device and does not require a
          deck or bundle.
        </p>
      </Card.Content>
    </Card.Root>
  {/if}
</section>

<style>
  .typing-screen {
    display: grid;
    min-width: 0;
    gap: 1.25rem;
  }

  .typing-screen :global(.screen-header) {
    margin-bottom: 0.25rem;
  }

  .typing-settings {
    display: grid;
    grid-template-columns: minmax(0, 2fr) minmax(15rem, 1fr);
    gap: 1rem;
  }

  .choice-fieldset {
    min-width: 0;
    margin: 0;
    padding: 1rem;
    border: 1px solid var(--border);
  }

  .choice-fieldset legend {
    padding: 0 0.35rem;
    font-size: var(--text-sm);
    font-weight: 800;
  }

  .language-choices {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.5rem;
  }

  .platform-fieldset,
  .platform-choices {
    display: grid;
    gap: 0.5rem;
  }

  .platform-choices {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .platform-choices :global(button) {
    min-width: 0;
  }

  @media (max-width: 760px) {
    .typing-settings,
    .language-choices {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
