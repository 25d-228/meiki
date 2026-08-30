export type TypingSpeechStatus = "unavailable" | "failed" | null;

type PendingPronunciation = {
  text: string;
  languageTag: string;
};

// A short phrase remains ordered, but rapid input cannot leave a stale audio backlog.
const maximumPendingPronunciations = 16;

export class TypingSpeech {
  private readonly synthesis: SpeechSynthesis | null;
  private readonly utteranceConstructor: typeof SpeechSynthesisUtterance | null;
  private readonly updateStatus: (status: TypingSpeechStatus) => void;
  private pending: PendingPronunciation[] = [];
  private activeUtterance: SpeechSynthesisUtterance | null = null;

  constructor(
    platformWindow: Window,
    updateStatus: (status: TypingSpeechStatus) => void,
  ) {
    this.synthesis = platformWindow.speechSynthesis ?? null;
    this.utteranceConstructor =
      (
        platformWindow as Window & {
          SpeechSynthesisUtterance?: typeof SpeechSynthesisUtterance;
        }
      ).SpeechSynthesisUtterance ?? null;
    this.updateStatus = updateStatus;
    this.synthesis?.addEventListener("voiceschanged", this.handleVoicesChanged);
  }

  pronounce(texts: string[], languageTag: string): void {
    const pronunciations = texts
      .filter((text) => /\S/u.test(text))
      .map((text) => ({ text, languageTag }));
    if (pronunciations.length === 0) return;
    if (!this.synthesis || !this.utteranceConstructor) {
      this.updateStatus("unavailable");
      return;
    }

    this.pending = [...this.pending, ...pronunciations].slice(
      -maximumPendingPronunciations,
    );
    this.startNext();
  }

  cancel(): void {
    this.pending = [];
    this.activeUtterance = null;
    this.synthesis?.cancel();
    this.updateStatus(null);
  }

  destroy(): void {
    this.cancel();
    this.synthesis?.removeEventListener(
      "voiceschanged",
      this.handleVoicesChanged,
    );
  }

  private readonly handleVoicesChanged = (): void => {
    if (this.pending.length > 0) this.startNext();
  };

  private startNext(): void {
    if (
      !this.synthesis ||
      !this.utteranceConstructor ||
      this.activeUtterance ||
      this.pending.length === 0
    ) {
      return;
    }

    const pronunciation = this.pending[0];
    const voice = matchingLocalVoice(
      this.synthesis.getVoices(),
      pronunciation.languageTag,
    );
    if (!voice) {
      this.updateStatus("unavailable");
      return;
    }

    this.pending = this.pending.slice(1);
    try {
      const utterance = new this.utteranceConstructor(pronunciation.text);
      utterance.lang = pronunciation.languageTag;
      utterance.voice = voice;
      utterance.onend = () => this.finish(utterance, null);
      utterance.onerror = () => this.finish(utterance, "failed");
      this.activeUtterance = utterance;
      this.updateStatus(null);
      this.synthesis.speak(utterance);
    } catch {
      this.activeUtterance = null;
      this.pending = [];
      this.updateStatus("failed");
    }
  }

  private finish(
    utterance: SpeechSynthesisUtterance,
    status: TypingSpeechStatus,
  ): void {
    if (this.activeUtterance !== utterance) return;
    this.activeUtterance = null;
    if (status === "failed") {
      this.pending = [];
      this.updateStatus(status);
      return;
    }
    this.startNext();
  }
}

function matchingLocalVoice(
  voices: SpeechSynthesisVoice[],
  languageTag: string,
): SpeechSynthesisVoice | null {
  const localVoices = voices.filter((voice) => voice.localService);
  const normalizedLanguageTag = languageTag.toLowerCase();
  const exactMatch = localVoices.find(
    (voice) => voice.lang.toLowerCase() === normalizedLanguageTag,
  );
  if (exactMatch) return exactMatch;

  const primaryLanguage = normalizedLanguageTag.split("-")[0];
  return (
    localVoices.find(
      (voice) => voice.lang.toLowerCase().split("-")[0] === primaryLanguage,
    ) ?? null
  );
}
