// apps/annotation_studio/src/api/queries.ts — TanStack Query Hooks
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  fetchDomains,
  fetchSamples,
  saveGroundTruth,
  partitionDataset,
  runInference,
  fetchFloodDelta,
  type SaveTruthPayload,
  type DatasetSplitPayload,
  type InferencePayload,
} from './client';

export const queryKeys = {
  domains: ['domains'] as const,
  samples: (domainId: string) => ['samples', domainId] as const,
  floodDelta: ['flood_delta'] as const,
};

export function useDomainsQuery() {
  return useQuery({
    queryKey: queryKeys.domains,
    queryFn: fetchDomains,
    staleTime: 1000 * 60 * 10,
  });
}

export function useSamplesQuery(domainId: string | null) {
  return useQuery({
    queryKey: queryKeys.samples(domainId || ''),
    queryFn: () => fetchSamples(domainId!),
    enabled: Boolean(domainId),
    staleTime: 1000 * 60 * 2,
  });
}

export function useSaveTruthMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (payload: SaveTruthPayload) => saveGroundTruth(payload),
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: queryKeys.samples(variables.domain) });
    },
  });
}

export function useDatasetSplitMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (payload: DatasetSplitPayload) => partitionDataset(payload),
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: queryKeys.samples(variables.domain) });
    },
  });
}

export function useInferenceMutation() {
  return useMutation({
    mutationFn: (payload: InferencePayload) => runInference(payload),
  });
}

export function useFloodDeltaQuery(enabled = false) {
  return useQuery({
    queryKey: queryKeys.floodDelta,
    queryFn: fetchFloodDelta,
    enabled,
    staleTime: 1000 * 60 * 5,
  });
}
