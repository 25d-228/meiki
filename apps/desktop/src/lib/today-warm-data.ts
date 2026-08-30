import type { TodayOverviewDto } from "./generated/TodayOverviewDto";
import type { TodayStatisticsDto } from "./generated/TodayStatisticsDto";

export type TodayWarmData = {
  deckId: string;
  dayStartMs: number;
  dayEndMs: number;
  dayBoundaryMinutes: number;
  overview: TodayOverviewDto;
  statistics: TodayStatisticsDto | null;
};
