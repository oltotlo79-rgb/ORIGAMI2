import type { FormEventHandler } from 'react'
import { APP_TEXT } from '../lib/appText.ts'
import type { ProjectSnapshot } from '../lib/coreClient.ts'
import {
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import type { useBeginnerCandidateWorkflow } from '../lib/useBeginnerCandidateWorkflow.ts'
import type { useBeginnerParameterGridWorkflow } from '../lib/useBeginnerParameterGridWorkflow.ts'
import { BeginnerCandidateControls } from './BeginnerCandidateControls.tsx'
import { BeginnerCandidateResults } from './BeginnerCandidateResults.tsx'

type CandidateWorkflow = ReturnType<typeof useBeginnerCandidateWorkflow>
type GridWorkflow = ReturnType<typeof useBeginnerParameterGridWorkflow>

export function ProjectMemoAndCandidateSection({
  locale,
  snapshot,
  coreBusy,
  recoveryBlocking,
  skeletonTreeStatus,
  candidateWorkflow,
  gridWorkflow,
  onSubmitMemo,
}: Readonly<{
  locale: Locale
  snapshot: ProjectSnapshot
  coreBusy: boolean
  recoveryBlocking: boolean
  skeletonTreeStatus: string
  candidateWorkflow: CandidateWorkflow
  gridWorkflow: GridWorkflow
  onSubmitMemo: FormEventHandler<HTMLFormElement>
}>) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )

  return (
    <section className="property-section">
      <h2>{text(APP_TEXT.projectMemo)}</h2>
      <form
        key={`${snapshot.project_instance_id}:${snapshot.memo}`}
        onSubmit={onSubmitMemo}
      >
        <label>
          <span>{text(APP_TEXT.notes)}</span>
          <textarea
            name="project_memo"
            maxLength={16_000}
            rows={5}
            defaultValue={snapshot.memo}
            disabled={coreBusy || recoveryBlocking}
          />
        </label>
        <div className="property-actions">
          <button type="submit" disabled={coreBusy || recoveryBlocking}>
            {text(APP_TEXT.saveMemo)}
          </button>
        </div>
      </form>
      <div aria-labelledby="beginner-candidate-heading">
        <BeginnerCandidateControls
          locale={locale}
          coreBusy={coreBusy}
          recoveryBlocking={recoveryBlocking}
          skeletonTreeStatus={skeletonTreeStatus}
          candidateWorkflow={candidateWorkflow}
          gridWorkflow={gridWorkflow}
        />
        <BeginnerCandidateResults
          locale={locale}
          snapshot={snapshot}
          coreBusy={coreBusy}
          recoveryBlocking={recoveryBlocking}
          candidateWorkflow={candidateWorkflow}
        />
      </div>
    </section>
  )
}
