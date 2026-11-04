import { SuitesList } from "@/components/suites/SuitesList";
import { SuitesShell } from "@/components/suites/SuitesShell";

export const dynamic = "force-dynamic";

export default function SuitesPage() {
  return (
    <SuitesShell>
      <p className="mt-2 text-sm text-zinc-400 max-w-3xl">
        Build your own eval suite from merged pull requests in a GitHub repository you can read. A model you pick
        writes each task&apos;s issue and hidden tests, the validator checks them, you review and accept them, and the
        frozen suite runs on the Launch page like VulcanBench Frontier v4.
      </p>
      <SuitesList />
    </SuitesShell>
  );
}
