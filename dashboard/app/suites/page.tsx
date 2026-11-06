import { SuitesList } from "@/components/suites/SuitesList";
import { SuitesShell } from "@/components/suites/SuitesShell";

export const dynamic = "force-dynamic";

export default function SuitesPage() {
  return (
    <SuitesShell
      intro={
        <p className="mt-2 text-sm text-zinc-400 max-w-3xl">
          An eval suite is a set of coding tasks a model is scored on. Build your own from merged pull requests in a
          GitHub repository you can read: each pull request becomes a task where the model has to make the same fix
          from an issue, without seeing the original change. Pick the pull requests, let a model write each task&apos;s
          issue and hidden tests, review and accept the good ones, then freeze the suite and benchmark it from the
          Launch page like VulcanBench Frontier v4.
        </p>
      }
    >
      <SuitesList />
    </SuitesShell>
  );
}
