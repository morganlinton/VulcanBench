import { SuitesList } from "@/components/suites/SuitesList";
import { SuitesShell } from "@/components/suites/SuitesShell";

export const dynamic = "force-dynamic";

export default function SuitesPage() {
  return (
    <SuitesShell
      intro={
        <p className="mt-2 text-sm text-zinc-400 max-w-3xl">
          Turn merged pull requests from your own GitHub repository into an eval suite, then benchmark models on it
          from the Launch page.
        </p>
      }
    >
      <SuitesList />
    </SuitesShell>
  );
}
