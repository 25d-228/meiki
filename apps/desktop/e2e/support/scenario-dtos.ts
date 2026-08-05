const dueAt = "2026-07-30T09:00:00+00:00";
const nextDueAt = "2026-08-01T09:00:00+00:00";

const media = {
  prompt_audio: {
    id: "prompt-audio-fixture",
    content_hash: `sha256:${"a".repeat(64)}`,
    kind: "audio",
    role: "prompt_audio",
    media_type: "audio/wav",
    byte_size: 44,
    original_file_name: "prompt.wav",
    alt_text: null,
    width: null,
    height: null,
    duration_ms: 1000,
    language_tag: "ja",
    direction: "auto",
    asset_path:
      "data:audio/wav;base64,UklGRiQAAABXQVZFZm10IBAAAAABAAEARKwAAIhYAQACABAAZGF0YQAAAAA=",
    availability: "ready",
  },
  answer_audio: {
    id: "answer-audio-fixture",
    content_hash: `sha256:${"b".repeat(64)}`,
    kind: "audio",
    role: "answer_audio",
    media_type: "audio/wav",
    byte_size: 44,
    original_file_name: "answer.wav",
    alt_text: null,
    width: null,
    height: null,
    duration_ms: 1000,
    language_tag: "ja",
    direction: "auto",
    asset_path:
      "data:audio/wav;base64,UklGRiQAAABXQVZFZm10IBAAAAABAAEARKwAAIhYAQACABAAZGF0YQAAAAA=",
    availability: "ready",
  },
  reveal_image: {
    id: "reveal-image-fixture",
    content_hash: `sha256:${"c".repeat(64)}`,
    kind: "image",
    role: "reveal_image",
    media_type: "image/png",
    byte_size: 68,
    original_file_name: "library.png",
    alt_text: "A quiet library reading room",
    width: 1,
    height: 1,
    duration_ms: null,
    language_tag: "ja",
    direction: "auto",
    asset_path:
      "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
    availability: "ready",
  },
} as const;

const content = {
  cjk: {
    prompt: "日曜日は図書館に[…]",
    fullSource: "日曜日は図書館に行きます",
    answer: "行きます",
    rawResponse: "行きます",
    normalizedResponse: "行きます",
    languageTag: "ja",
    direction: "auto",
  },
  devanagari: {
    prompt: "मैं […] पढ़ता हूँ",
    fullSource: "मैं पुस्तक पढ़ता हूँ",
    answer: "पुस्तक",
    rawResponse: "पुस्तक",
    normalizedResponse: "पुस्तक",
    languageTag: "hi",
    direction: "ltr",
  },
  emoji: {
    prompt: "Family: […]",
    fullSource: "Family: 👨‍👩‍👧‍👦",
    answer: "👨‍👩‍👧‍👦",
    rawResponse: "👨‍👩‍👧‍👦",
    normalizedResponse: "👨‍👩‍👧‍👦",
    languageTag: null,
    direction: "ltr",
  },
  ltr: {
    prompt: "Le dimanche, je vais à […]",
    fullSource: "Le dimanche, je vais à la bibliothèque",
    answer: "la bibliothèque",
    rawResponse: " la bibliothèque ",
    normalizedResponse: "la bibliothèque",
    languageTag: "fr",
    direction: "ltr",
  },
  rtl: {
    prompt: "من هر روز […] می‌خوانم",
    fullSource: "من هر روز کتاب می‌خوانم",
    answer: "کتاب",
    rawResponse: "کتاب",
    normalizedResponse: "کتاب",
    languageTag: "fa",
    direction: "rtl",
  },
  mixed: {
    prompt: "Meetingは الساعة […] に始まる",
    fullSource: "Meetingは الساعة 三時 に始まる",
    answer: "三時",
    rawResponse: "三時",
    normalizedResponse: "三時",
    languageTag: null,
    direction: "auto",
  },
  longmixed: {
    prompt:
      "Meetingは الساعة […] に始まる — this deliberately long multilingual prompt keeps 日本語, العربية, and English context readable without horizontal scrolling.",
    fullSource:
      "Meetingは الساعة 三時 に始まる — this deliberately long multilingual prompt keeps 日本語, العربية, and English context readable without horizontal scrolling.",
    answer: "三時",
    rawResponse: "三時",
    normalizedResponse: "三時",
    languageTag: null,
    direction: "auto",
  },
} as const;

type StudyContent = (typeof content)[keyof typeof content];

function studyCard(value: StudyContent, cardId = "due-card") {
  return {
    card_id: cardId,
    card_content_version: 0,
    schedule_version: 0,
    prompt:
      cardId === "new-card" ? `Second card · ${value.prompt}` : value.prompt,
    language_tag: value.languageTag,
    direction: value.direction,
    due_at: dueAt,
    completed_reviews: 0,
    suspended: false,
    hint: null,
    prompt_media: [],
  };
}

function reveal(value: StudyContent) {
  const answerStart = value.fullSource.indexOf(value.answer);
  return {
    card_id: "due-card",
    card_content_version: 0,
    schedule_version: 0,
    full_source: value.fullSource,
    source_segments: [
      {
        text: value.fullSource.slice(0, answerStart),
        highlighted: false,
      },
      { text: value.answer, highlighted: true },
      {
        text: value.fullSource.slice(answerStart + value.answer.length),
        highlighted: false,
      },
    ],
    expected_answer: value.answer,
    raw_response: value.rawResponse,
    normalized_response: value.normalizedResponse,
    comparison: "exact",
    difference: [{ kind: "equal", text: value.answer }],
    suggested_grade: "good",
    grade_previews: [
      { grade: "again", due_at: dueAt, interval_seconds: 60 },
      { grade: "hard", due_at: dueAt, interval_seconds: 3600 },
      { grade: "good", due_at: nextDueAt, interval_seconds: 259200 },
      { grade: "easy", due_at: nextDueAt, interval_seconds: 604800 },
    ],
    annotations: [],
    explanation: null,
    answer_media: [],
  };
}

const decks = [
  {
    id: "default-deck",
    name: "Japanese",
    is_default: true,
    note_count: 1,
    daily_time_budget_override_minutes: null,
    language_tag: "ja",
    direction: "auto",
    matching_policy: "strict",
  },
  {
    id: "travel-deck",
    name: "Travel phrases",
    is_default: false,
    note_count: 1,
    daily_time_budget_override_minutes: null,
    language_tag: null,
    direction: "auto",
    matching_policy: "strict",
  },
] as const;

const deckSummaries = [
  {
    id: "default-deck",
    name: "Unsorted",
    total_cards: 3,
    due_cards: 1,
    new_cards: 1,
  },
  {
    id: "travel-deck",
    name: "Travel phrases",
    total_cards: 1,
    due_cards: 0,
    new_cards: 1,
  },
] as const;

const queue = [
  {
    card_id: "due-card",
    deck_id: "default-deck",
    card_content_version: 0,
    schedule_version: 0,
    due_at: dueAt,
    ideal_due_at: dueAt,
    overdue: false,
    is_new: false,
  },
  {
    card_id: "new-card",
    deck_id: "default-deck",
    card_content_version: 0,
    schedule_version: 0,
    due_at: dueAt,
    ideal_due_at: dueAt,
    overdue: false,
    is_new: true,
  },
] as const;

function todayOverview(
  values: Partial<{
    due_reviews: number;
    overdue_reviews: number;
    new_cards: number;
    deferred_new_cards: number;
    estimated_seconds: number;
    backlog_exceeds_budget: boolean;
    daily_time_budget_minutes: number | null;
    next_due_at: string | null;
    queue: readonly unknown[];
  }> = {},
) {
  return {
    deck_id: "__all_decks__",
    deck_name: "All decks",
    decks: decks.map(({ id, name }) => ({ id, name })),
    due_reviews: 1,
    overdue_reviews: 0,
    new_cards: 1,
    deferred_new_cards: 0,
    estimated_seconds: 50,
    estimate_uses_history: true,
    response_time_samples: 8,
    daily_time_budget_minutes: 30,
    budget_source: "collection_budget",
    target_retention_basis_points: 9000,
    policy_explanation:
      "30 min/day\nTarget retention: 90%\nNew cards today: 1\nReason: fixture response.",
    backlog_exceeds_budget: false,
    next_due_at: null,
    queue,
    ...values,
  };
}

const emptyDraft = {
  source_id: "source-fixture",
  deck_id: "default-deck",
  persisted: false,
  created_at_ms: 1_700_000_000_000,
  deck_language_tag: null,
  deck_direction: "auto",
  deck_matching_policy: "strict",
  language_tag: null,
  direction: "auto",
  segments: [
    {
      id: "segment-fixture",
      ordinal: 0,
      kind: "text",
      text: "",
      cloze_id: null,
    },
  ],
  clozes: [],
  active_cloze_id: null,
};

function authoredDraft(
  source: string,
  answer: string,
  direction: string,
  deckId = "default-deck",
) {
  const start = source.indexOf(answer);
  const before = source.slice(0, start);
  const after = source.slice(start + answer.length);
  return {
    ...emptyDraft,
    deck_id: deckId,
    direction,
    segments: [
      ...(before
        ? [
            {
              id: "segment-before",
              ordinal: 0,
              kind: "text",
              text: before,
              cloze_id: null,
            },
          ]
        : []),
      {
        id: "segment-cloze",
        ordinal: before ? 1 : 0,
        kind: "cloze",
        text: answer,
        cloze_id: "cloze-fixture",
      },
      ...(after
        ? [
            {
              id: "segment-after",
              ordinal: before ? 2 : 1,
              kind: "text",
              text: after,
              cloze_id: null,
            },
          ]
        : []),
    ],
    clozes: [
      {
        id: "cloze-fixture",
        card_id: "card-fixture",
        answer,
        accepted_answers: [],
        hint: "",
        language_tag: null,
        direction,
        matching_policy: null,
        annotations: [],
        explanation_markdown: "",
        media: [],
      },
    ],
    active_cloze_id: "cloze-fixture",
  };
}

const cjkDraft = authoredDraft("日曜日は図書館に行きます", "図書館", "auto");

const libraryNotes = [
  {
    source_id: "sample-source",
    deck_id: "default-deck",
    deck_name: "Japanese",
    source_text: "日曜日は図書館に行きます",
    language_tag: "ja",
    direction: "auto",
    tags: [{ id: "tag-travel", name: "旅行" }],
    cards: [
      {
        card_id: "due-card",
        cloze_id: "sample-cloze",
        prompt: "日曜日は図書館に[…]",
        answer: "行きます",
        suspended: false,
        is_new: false,
        is_due: true,
        due_at: dueAt,
        language_tag: "ja",
        direction: "auto",
        has_media: true,
      },
    ],
    media_count: 1,
    deleted: false,
    deleted_at: null,
    updated_at_ms: 1,
  },
  {
    source_id: "source-ar",
    deck_id: "travel-deck",
    deck_name: "Travel phrases",
    source_text: "أنا أقرأ كتابًا في المكتبة",
    language_tag: "ar",
    direction: "rtl",
    tags: [{ id: "tag-arabic", name: "العربية" }],
    cards: [
      {
        card_id: "card-ar",
        cloze_id: "cloze-ar",
        prompt: "أنا أقرأ […] في المكتبة",
        answer: "كتابًا",
        suspended: false,
        is_new: false,
        is_due: false,
        due_at: nextDueAt,
        language_tag: "ar",
        direction: "rtl",
        has_media: false,
      },
    ],
    media_count: 0,
    deleted: false,
    deleted_at: null,
    updated_at_ms: 2,
  },
] as const;

const library = {
  notes: libraryNotes,
  decks: decks.map(({ id, name }) => ({ id, name })),
  tags: [
    { id: "tag-travel", name: "旅行" },
    { id: "tag-arabic", name: "العربية" },
  ],
  languages: ["ar", "ja"],
  total_matches: 2,
  active_notes: 2,
  trashed_notes: 0,
  offset: 0,
  limit: 50,
};

const deckCards = {
  default: {
    cards: [
      {
        id: "due-card",
        sentence: "日曜日は図書館に[…]",
        answer: "行きます",
        status: "due",
        language_tag: "ja",
        direction: "auto",
      },
    ],
    decks: [
      { id: "default-deck", name: "Unsorted" },
      { id: "travel-deck", name: "Travel phrases" },
    ],
    total_matches: 1,
    offset: 0,
    limit: 25,
  },
  travel: {
    cards: [
      {
        id: "card-ar",
        sentence: "أنا أقرأ […] في المكتبة",
        answer: "كتابًا",
        status: "scheduled",
        language_tag: "ar",
        direction: "rtl",
      },
      {
        id: "travel-new-card",
        sentence: "Take the […] train",
        answer: "express",
        status: "new",
        language_tag: "en",
        direction: "ltr",
      },
    ],
    decks: [
      { id: "default-deck", name: "Unsorted" },
      { id: "travel-deck", name: "Travel phrases" },
    ],
    total_matches: 2,
    offset: 0,
    limit: 25,
  },
  trash: {
    cards: [
      {
        id: "trashed-card",
        sentence: "Recover this […]",
        answer: "card",
        status: "suspended",
        language_tag: "en",
        direction: "ltr",
      },
    ],
    decks: [
      { id: "default-deck", name: "Unsorted" },
      { id: "travel-deck", name: "Travel phrases" },
    ],
    total_matches: 1,
    offset: 0,
    limit: 25,
  },
};

const schedulerSettings = {
  deck_id: "default-deck",
  scheduling_mode: "automatic",
  collection_daily_time_budget_minutes: 30,
  deck_daily_time_budget_minutes: null,
  effective_daily_time_budget_minutes: 30,
  budget_source: "collection_budget",
  target_retention_basis_points: 9000,
  new_cards_per_day: 20,
  maximum_interval_days: 36500,
  day_boundary_minutes: 240,
  controller_backlog_exceeds_budget: false,
  controller_explanation:
    "30 min/day\nTarget retention: 90%\nNew cards today: 20\nReason: fixture response.",
};

export const scenarioDtos = {
  media,
  study: Object.fromEntries(
    Object.entries(content).map(([name, value]) => [
      name,
      {
        first: studyCard(value),
        second: studyCard(value, "new-card"),
        reveal: reveal(value),
      },
    ]),
  ),
  missingMediaCard: {
    ...studyCard(content.cjk),
    prompt_media: [
      {
        ...media.prompt_audio,
        asset_path: null,
        availability: "missing",
      },
    ],
  },
  readyMediaCard: {
    ...studyCard(content.cjk),
    prompt_media: [media.prompt_audio],
  },
  readyMediaReveal: {
    ...reveal(content.cjk),
    answer_media: [media.answer_audio, media.reveal_image],
  },
  wrongReveal: {
    ...reveal(content.cjk),
    raw_response: " 図書館 ",
    normalized_response: "図書館",
    comparison: "incorrect",
    difference: [
      { kind: "delete", text: "行きます" },
      { kind: "insert", text: "図書館" },
    ],
    suggested_grade: "again",
  },
  today: {
    normal: todayOverview(),
    empty: todayOverview({
      due_reviews: 0,
      new_cards: 0,
      estimated_seconds: 0,
      next_due_at: nextDueAt,
      queue: [],
    }),
    overdue: todayOverview({
      due_reviews: 2,
      overdue_reviews: 1,
      queue: [
        { ...queue[0], card_id: "overdue-card", overdue: true },
        queue[0],
        queue[1],
      ],
    }),
    capped: todayOverview({
      deferred_new_cards: 2,
      daily_time_budget_minutes: 1,
    }),
    backlog: todayOverview({ backlog_exceeds_budget: true }),
    budget: todayOverview({
      new_cards: 3,
      estimated_seconds: 110,
      queue: [
        queue[0],
        queue[1],
        { ...queue[1], card_id: "new-card-2" },
        { ...queue[1], card_id: "new-card-3" },
      ],
    }),
  },
  emptyCollectionPlan: {
    availability: "empty_collection",
    overview: todayOverview({
      due_reviews: 0,
      new_cards: 0,
      estimated_seconds: 0,
      next_due_at: null,
      queue: [],
    }),
  },
  nothingDuePlan: {
    availability: "nothing_due",
    overview: todayOverview({
      due_reviews: 0,
      new_cards: 0,
      estimated_seconds: 0,
      next_due_at: nextDueAt,
      queue: [],
    }),
  },
  readyPlan: {
    availability: "ready",
    overview: todayOverview(),
  },
  reconciledQueue: queue.map(
    ({ card_id, card_content_version, schedule_version }) => ({
      card_id,
      card_content_version,
      schedule_version,
    }),
  ),
  reconciledSecondCard: [
    {
      card_id: "new-card",
      card_content_version: 0,
      schedule_version: 0,
    },
  ],
  gradeResult: {
    review_event_id: "REPLACED_BY_REQUEST",
    schedule_version: 1,
    due_at: nextDueAt,
    interval_seconds: 259200,
  },
  undoResult: {
    undo_event_id: "REPLACED_BY_REQUEST",
    schedule_version: 2,
    due_at: dueAt,
    interval_seconds: 0,
    completed_reviews: 0,
  },
  suspendedCard: {
    ...studyCard(content.cjk),
    suspended: true,
  },
  decks,
  deckSummaries,
  createdDeck: {
    ...decks[1],
    id: "listening-deck",
    name: "Listening",
    note_count: 0,
  },
  renamedDeck: {
    ...decks[1],
    name: "Audio",
  },
  deletedDeck: {
    deleted_deck_id: "listening-deck",
    moved_notes: 0,
  },
  movedDeck: {
    deleted_deck_id: "travel-deck",
    moved_notes: 2,
  },
  deckLifecycle: [
    decks,
    decks,
    [
      decks[0],
      {
        ...decks[1],
        name: "Audio",
      },
    ],
    [decks[0]],
  ],
  deckSummaryLifecycle: [
    deckSummaries,
    [
      ...deckSummaries,
      {
        id: "listening-deck",
        name: "Listening",
        total_cards: 0,
        due_cards: 0,
        new_cards: 0,
      },
    ],
  ],
  schedulerSettings,
  midnightSchedulerSettings: {
    ...schedulerSettings,
    day_boundary_minutes: 0,
  },
  schedulerPreview: {
    effective_daily_time_budget_minutes: 60,
    budget_source: "collection_budget",
    target_retention_basis_points: 9000,
    new_cards_per_day: 20,
    backlog_exceeds_budget: false,
    explanation:
      "60 min/day\nTarget retention: 90%\nNew cards today: 20\nReason: fixture response.",
  },
  expertSchedulerPreview: {
    effective_daily_time_budget_minutes: 60,
    budget_source: "collection_budget",
    target_retention_basis_points: 8750,
    new_cards_per_day: 12,
    backlog_exceeds_budget: false,
    explanation:
      "60 min/day\nTarget retention: 87.5%\nNew cards today: 12\nReason: fixture response.",
  },
  savedAutomaticSettings: {
    ...schedulerSettings,
    collection_daily_time_budget_minutes: 60,
    effective_daily_time_budget_minutes: 60,
  },
  savedExpertSettings: {
    ...schedulerSettings,
    scheduling_mode: "expert",
    collection_daily_time_budget_minutes: 60,
    effective_daily_time_budget_minutes: 60,
    target_retention_basis_points: 8750,
    new_cards_per_day: 12,
    controller_explanation:
      "60 min/day\nTarget retention: 87.5%\nNew cards today: 12\nReason: fixture response.",
  },
  library,
  deckCards,
  emptyLibrary: {
    ...library,
    notes: [],
    tags: [],
    languages: [],
    total_matches: 0,
    active_notes: 0,
  },
  bulkResults: {
    suspend: { affected_notes: 2, action: "suspend", undo_action: null },
    unsuspend: { affected_notes: 2, action: "unsuspend", undo_action: null },
    delete: { affected_notes: 2, action: "delete", undo_action: "restore" },
    restore: { affected_notes: 2, action: "restore", undo_action: "delete" },
    move: { affected_notes: 2, action: "move", undo_action: null },
    add_tag: { affected_notes: 2, action: "add_tag", undo_action: null },
    remove_tag: { affected_notes: 2, action: "remove_tag", undo_action: null },
  },
  emptyDraft,
  authoring: {
    cjk: cjkDraft,
    rtl: authoredDraft("أنا أقرأ كتابًا", "كتابًا", "rtl"),
    devanagari: authoredDraft("मैं पुस्तक पढ़ता हूँ", "पुस्तक", "ltr"),
    ltr: authoredDraft("Réviser le café", "café", "ltr"),
    han: authoredDraft("学习漢字", "漢字", "auto"),
    mixed: authoredDraft("Meetingは الساعة 三時", "三時", "auto"),
    listening: authoredDraft(
      "Listen carefully",
      "Listen",
      "auto",
      "travel-deck",
    ),
    media: authoredDraft("図書館", "図書館", "auto"),
    removed: {
      ...cjkDraft,
      segments: [
        {
          id: "segment-fixture",
          ordinal: 0,
          kind: "text",
          text: "日曜日は図書館に行きます",
          cloze_id: null,
        },
      ],
      clozes: [],
      active_cloze_id: null,
    },
  },
  authoringPreviews: {
    cjk: [
      {
        cloze_id: "cloze-fixture",
        prompt: "日曜日は[…]に行きます",
        answer: "図書館",
        language_tag: null,
        direction: "auto",
        hint: "",
        annotations: [],
        explanation_markdown: "**Use** the fixture explanation.",
      },
    ],
    rtl: [
      {
        cloze_id: "cloze-fixture",
        prompt: "أنا أقرأ […]",
        answer: "كتابًا",
        language_tag: "ar",
        direction: "rtl",
        hint: "",
        annotations: [],
        explanation_markdown: "",
      },
    ],
    devanagari: [
      {
        cloze_id: "cloze-fixture",
        prompt: "मैं […] पढ़ता हूँ",
        answer: "पुस्तक",
        language_tag: "hi",
        direction: "ltr",
        hint: "",
        annotations: [],
        explanation_markdown: "",
      },
    ],
    ltr: [
      {
        cloze_id: "cloze-fixture",
        prompt: "Réviser le […]",
        answer: "café",
        language_tag: "fr",
        direction: "ltr",
        hint: "",
        annotations: [],
        explanation_markdown: "",
      },
    ],
    han: [
      {
        cloze_id: "cloze-fixture",
        prompt: "学习[…]",
        answer: "漢字",
        language_tag: "zh",
        direction: "auto",
        hint: "",
        annotations: [],
        explanation_markdown: "",
      },
    ],
    mixed: [
      {
        cloze_id: "cloze-fixture",
        prompt: "Meetingは الساعة […]",
        answer: "三時",
        language_tag: null,
        direction: "auto",
        hint: "",
        annotations: [],
        explanation_markdown: "",
      },
    ],
    listening: [
      {
        cloze_id: "cloze-fixture",
        prompt: "[…] carefully",
        answer: "Listen",
        language_tag: null,
        direction: "auto",
        hint: "",
        annotations: [],
        explanation_markdown: "",
      },
    ],
    media: [
      {
        cloze_id: "cloze-fixture",
        prompt: "[…]",
        answer: "図書館",
        language_tag: null,
        direction: "auto",
        hint: "",
        annotations: [],
        explanation_markdown: "",
      },
    ],
  },
  persistedDraft: {
    ...authoredDraft("日曜日は図書館に行きます", "行きます", "auto"),
    source_id: "sample-source",
    persisted: true,
    language_tag: "ja",
    deck_language_tag: "ja",
  },
  archiveExport: {
    path: "/tmp/exports/meiki-e2e.meiki",
    decks: 2,
    notes: 2,
    cards: 2,
    review_events: 1,
    media_objects: 1,
  },
  archivePreview: {
    path: "/tmp/exports/meiki-e2e.meiki",
    format_version: 4,
    deck_name: "Japanese Foundation 1",
    decks: 1,
    notes: 1,
    cards: 1,
    review_events: 0,
    media_objects: 1,
    duplicate_media_objects: 1,
    can_add_deck: true,
    add_deck_summary:
      'Ready to add deck "Japanese Foundation 1" with 1 note(s), 1 card(s), and 1 media object(s).',
    can_import: true,
    confirmation: "REPLACE",
    summary: "Validated 1 note(s), 1 card(s), and 1 media object(s).",
  },
  archiveAddDeck: {
    backup_path: "/tmp/backups/collection.db.pre-add-deck.bak",
    deck_id: "japanese-foundation-1",
    deck_name: "Japanese Foundation 1",
    imported_notes: 1,
    imported_cards: 1,
    imported_media_objects: 0,
    deduplicated_media_objects: 1,
  },
  archiveImport: {
    backup_path: "/tmp/backups/collection.db.pre-import.bak",
    imported_notes: 2,
    imported_cards: 2,
    imported_media_objects: 0,
    deduplicated_media_objects: 1,
  },
} as const;
