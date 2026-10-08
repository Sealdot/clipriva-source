import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { PropsWithChildren } from "react";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Search terms create distinct cache entries. Clipboard values can be
      // comparatively large, so discard inactive results promptly instead of
      // retaining React Query's five-minute default working set.
      gcTime: 60_000,
      staleTime: 2_000,
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});

export function AppProviders({ children }: PropsWithChildren) {
  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
}
