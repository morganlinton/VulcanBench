import Link from "next/link";
import { RunDetail } from "@/components/runner/RunDetail";

export const dynamic = "force-dynamic";

interface Props {
  params: Promise<{ id: string }>;
}

export default async function RunDetailPage({ params }: Props) {
  const { id } = await params;
  let decoded = id;
  try {
    decoded = decodeURIComponent(id);
  } catch {
    decoded = id;
  }
  return (
    <div className="min-h-screen bg-zinc-950 text-white p-8 font-sans">
      <div className="max-w-6xl mx-auto">
        <Link href="/runs" className="text-sm text-zinc-400 hover:text-white">← runs</Link>
        <RunDetail id={decoded} />
      </div>
    </div>
  );
}
