import { listen } from '@tauri-apps/api/event'
import {
  useEffect,
  useRef,
  useState,
} from 'react'

import {
  applyBeginnerGeneratedPlan,
  applyBeginnerSymmetricParameters,
  appendGenericTreeInstructionProposal,
  cancelReferenceConsensus,
  evaluateBeginnerCandidates,
  getBeginnerSymmetricParameterEstimate,
  updateBeginnerDesignProfile,
  updateBeginnerReferenceConsensus,
  type BeginnerCandidateResponseV1,
  type BeginnerSymmetricParameterEstimateResponse,
  type ProjectSnapshot,
} from './coreClient.ts'
import type { LocalizedText } from './i18n.ts'
import {
  beginnerProjectBinding,
  matchesBeginnerProjectBinding,
  type BeginnerNativeEditRunner,
} from './beginnerWorkflowSupport.ts'

type ConsensusSelection = Readonly<{
  kind: 'image' | 'reference_model'
  asset_id: string
}>

type ConsensusProgress = Readonly<{
  processed_assets: number
  total_assets: number
  processed_pairs: number
  total_pairs: number
}>

type ConsensusProgressListener = (
  payload: Readonly<Record<string, unknown>>,
) => void

type CandidateTransport = Readonly<{
  evaluate: typeof evaluateBeginnerCandidates
  cancelConsensus: typeof cancelReferenceConsensus
  estimateSymmetric: typeof getBeginnerSymmetricParameterEstimate
  applySymmetric: typeof applyBeginnerSymmetricParameters
  applyPlan: typeof applyBeginnerGeneratedPlan
  appendInstructions: typeof appendGenericTreeInstructionProposal
  updateProfile: typeof updateBeginnerDesignProfile
  updateConsensus: typeof updateBeginnerReferenceConsensus
}>

const EMPTY_PROGRESS: ConsensusProgress = Object.freeze({
  processed_assets: 0,
  total_assets: 0,
  processed_pairs: 0,
  total_pairs: 0,
})

const DEFAULT_TRANSPORT: CandidateTransport = Object.freeze({
  evaluate: evaluateBeginnerCandidates,
  cancelConsensus: cancelReferenceConsensus,
  estimateSymmetric: getBeginnerSymmetricParameterEstimate,
  applySymmetric: applyBeginnerSymmetricParameters,
  applyPlan: applyBeginnerGeneratedPlan,
  appendInstructions: appendGenericTreeInstructionProposal,
  updateProfile: updateBeginnerDesignProfile,
  updateConsensus: updateBeginnerReferenceConsensus,
})

function defaultSubscribeConsensusProgress(
  listener: ConsensusProgressListener,
) {
  return listen<Record<string, unknown>>(
    'reference-consensus-progress-v1',
    (event) => listener(event.payload),
  )
}

export function useBeginnerCandidateWorkflow(input: Readonly<{
  snapshot: ProjectSnapshot | null
  getCurrentSnapshot: () => ProjectSnapshot | null
  runNativeEdit: BeginnerNativeEditRunner
  confirm: (message: LocalizedText) => boolean
  copy: Readonly<{
    applyPlan: LocalizedText
    saveSymmetric: LocalizedText
    appendInstructions: LocalizedText
  }>
  transport?: CandidateTransport
  createGenerationId?: () => string
  consensusProgressEnabled?: boolean
  subscribeConsensusProgress?: (
    listener: ConsensusProgressListener,
  ) => Promise<() => void>
}>) {
  const [beginnerCandidates, setBeginnerCandidates] =
    useState<BeginnerCandidateResponseV1 | null>(null)
  const [beginnerCandidateBusy, setBeginnerCandidateBusy] = useState(false)
  const [consensusProgress, setConsensusProgress] =
    useState<ConsensusProgress>(EMPTY_PROGRESS)
  const [selectedConsensusPair, setSelectedConsensusPair] =
    useState<string | null>(null)
  const [consensusSelectionDraft, setConsensusSelectionDraft] =
    useState<ConsensusSelection[]>([])
  const [beginnerSymmetricEstimate, setBeginnerSymmetricEstimate] =
    useState<BeginnerSymmetricParameterEstimateResponse | null>(null)
  const [beginnerSymmetricScale, setBeginnerSymmetricScale] = useState(25)
  const [beginnerSymmetricSpacing, setBeginnerSymmetricSpacing] = useState(35)
  const candidateRequestRef = useRef(0)
  const symmetricRequestRef = useRef(0)
  const consensusGenerationRef = useRef<string | null>(null)
  const busyRef = useRef(false)
  const snapshotRef = useRef(input.snapshot)
  snapshotRef.current = input.snapshot
  const snapshotProjectInstanceId = input.snapshot?.project_instance_id
  const snapshotRevision = input.snapshot?.revision
  const transport = input.transport ?? DEFAULT_TRANSPORT
  const transportRef = useRef(transport)
  transportRef.current = transport
  const createGenerationId =
    input.createGenerationId ?? (() => crypto.randomUUID())
  const subscribeConsensusProgress = input.subscribeConsensusProgress
    ?? defaultSubscribeConsensusProgress

  useEffect(() => {
    if (input.consensusProgressEnabled === false) return
    let disposed = false
    let unlisten: (() => void) | undefined
    void subscribeConsensusProgress((payload) => {
      if (
        disposed
        || payload.request_generation_id
          !== consensusGenerationRef.current
      ) return
      const values = [
        'processed_assets',
        'total_assets',
        'processed_pairs',
        'total_pairs',
      ].map((key) => Number(payload[key]))
      if (
        values.some(
          (value) => !Number.isInteger(value) || value < 0 || value > 6,
        )
        || values[0]! > values[1]!
        || values[2]! > values[3]!
      ) return
      setConsensusProgress({
        processed_assets: values[0]!,
        total_assets: values[1]!,
        processed_pairs: values[2]!,
        total_pairs: values[3]!,
      })
    }).then((dispose) => {
      if (disposed) dispose()
      else unlisten = dispose
    }).catch(() => undefined)
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [input.consensusProgressEnabled, subscribeConsensusProgress])

  useEffect(() => {
    candidateRequestRef.current += 1
    symmetricRequestRef.current += 1
    busyRef.current = false
    setBeginnerCandidateBusy(false)
    setBeginnerCandidates(null)
    const generationId = consensusGenerationRef.current
    consensusGenerationRef.current = null
    if (generationId) {
      void transportRef.current.cancelConsensus(generationId)
        .catch(() => undefined)
    }
    setConsensusProgress(EMPTY_PROGRESS)
    setSelectedConsensusPair(null)
    setConsensusSelectionDraft(
      (snapshotRef.current?.beginner_design_profile.reference_consensus_v1
        ?.bindings ?? []).map((binding) => ({
        kind: binding.kind,
        asset_id: binding.asset_id,
      })),
    )
    setBeginnerSymmetricEstimate(null)
  }, [snapshotProjectInstanceId, snapshotRevision])

  function requestBeginnerCandidates(requestedCandidateCount: number) {
    if (busyRef.current) return
    const current = input.getCurrentSnapshot()
    if (!current) return
    const binding = beginnerProjectBinding(current)
    const requestId = ++candidateRequestRef.current
    const generationId = createGenerationId()
    consensusGenerationRef.current = generationId
    busyRef.current = true
    setConsensusProgress(EMPTY_PROGRESS)
    setBeginnerCandidateBusy(true)
    void transport.evaluate(
      binding.project_id,
      binding.revision,
      binding.project_instance_id,
      requestedCandidateCount,
      generationId,
    ).then((response) => {
      if (
        candidateRequestRef.current !== requestId
        || consensusGenerationRef.current !== generationId
        || !matchesBeginnerProjectBinding(
          binding,
          input.getCurrentSnapshot(),
        )
        || !matchesBeginnerProjectBinding(response, current)
      ) return
      setBeginnerCandidates(response)
    }).catch(() => {
      if (
        candidateRequestRef.current === requestId
        && consensusGenerationRef.current === generationId
        && matchesBeginnerProjectBinding(
          binding,
          input.getCurrentSnapshot(),
        )
      ) setBeginnerCandidates(null)
    }).finally(() => {
      if (
        candidateRequestRef.current === requestId
        && consensusGenerationRef.current === generationId
      ) {
        busyRef.current = false
        setBeginnerCandidateBusy(false)
        consensusGenerationRef.current = null
      }
    })
  }

  function cancelCandidateRequest(clearCandidates: boolean) {
    candidateRequestRef.current += 1
    busyRef.current = false
    setBeginnerCandidateBusy(false)
    if (clearCandidates) setBeginnerCandidates(null)
    const generationId = consensusGenerationRef.current
    consensusGenerationRef.current = null
    if (generationId) {
      void transport.cancelConsensus(generationId).catch(() => undefined)
    }
  }

  function cancelConsensusAnalysis() {
    if (!consensusGenerationRef.current) return
    cancelCandidateRequest(false)
  }

  function cancelBeginnerCandidates() {
    cancelCandidateRequest(true)
  }

  function excludeBeginnerConsensusAsset(assetId: string | null) {
    const current = input.getCurrentSnapshot()
    const consensus =
      current?.beginner_design_profile.reference_consensus_v1
    if (
      !current
      || !consensus
      || (
        assetId !== null
        && !consensus.bindings.some(
          (binding) => binding.asset_id === assetId,
        )
      )
    ) return
    const profile = {
      ...current.beginner_design_profile,
      reference_consensus_v1: {
        ...consensus,
        ...(assetId === null
          ? { excluded_asset_id: undefined }
          : { excluded_asset_id: assetId }),
      },
    }
    void input.runNativeEdit((
      projectId,
      revision,
      projectInstanceId,
    ) => transport.updateProfile(
      projectId,
      revision,
      projectInstanceId,
      profile,
    ))
  }

  function toggleConsensusReference(
    kind: ConsensusSelection['kind'],
    assetId: string,
  ) {
    setConsensusSelectionDraft((current) => {
      const exists = current.some(
        (selection) => selection.asset_id === assetId,
      )
      if (exists) {
        return current.filter(
          (selection) => selection.asset_id !== assetId,
        )
      }
      if (current.length >= 4) return current
      return [...current, { kind, asset_id: assetId }]
    })
  }

  function saveConsensusReferences() {
    if (
      consensusSelectionDraft.length < 2
      || consensusSelectionDraft.length > 4
    ) return
    const canonical = [...consensusSelectionDraft].sort(
      (left, right) => left.asset_id.localeCompare(right.asset_id),
    )
    void input.runNativeEdit((
      projectId,
      revision,
      projectInstanceId,
    ) => transport.updateConsensus(
      projectId,
      revision,
      projectInstanceId,
      canonical,
    ))
  }

  function requestBeginnerSymmetricEstimate() {
    const current = input.getCurrentSnapshot()
    if (!current) return
    const binding = beginnerProjectBinding(current)
    const requestId = ++symmetricRequestRef.current
    void transport.estimateSymmetric(
      binding.project_id,
      binding.revision,
      binding.project_instance_id,
    ).then((response) => {
      if (
        requestId === symmetricRequestRef.current
        && matchesBeginnerProjectBinding(
          binding,
          input.getCurrentSnapshot(),
        )
        && matchesBeginnerProjectBinding(
          response,
          input.getCurrentSnapshot(),
        )
      ) {
        setBeginnerSymmetricEstimate(response)
        setBeginnerSymmetricScale(response.estimate.scale_percent)
        setBeginnerSymmetricSpacing(response.estimate.spacing_percent)
      }
    }).catch(() => {
      if (requestId === symmetricRequestRef.current) {
        setBeginnerSymmetricEstimate(null)
      }
    })
  }

  function confirmBeginnerSymmetricEstimate() {
    const estimate = beginnerSymmetricEstimate
    if (
      !estimate
      || !matchesBeginnerProjectBinding(
        estimate,
        input.getCurrentSnapshot(),
      )
      || !input.confirm(input.copy.saveSymmetric)
    ) return
    void input.runNativeEdit((
      projectId,
      revision,
      projectInstanceId,
    ) => transport.applySymmetric(
      projectId,
      revision,
      projectInstanceId,
      estimate.estimate,
      beginnerSymmetricScale,
      beginnerSymmetricSpacing,
    )).then((applied) => {
      if (applied) setBeginnerSymmetricEstimate(null)
    })
  }

  function confirmAndAppendGenericTreeInstructions() {
    const tree = input.getCurrentSnapshot()?.beginner_design_profile
      .generation_provenance?.generic_tree
    if (
      !tree?.instruction_proposal
      || !input.confirm(input.copy.appendInstructions)
    ) return
    void input.runNativeEdit((
      projectId,
      revision,
      projectInstanceId,
    ) => transport.appendInstructions(
      projectId,
      revision,
      projectInstanceId,
      tree.tree_topology_sha256,
    ))
  }

  function confirmAndApplyBeginnerPlan(
    kind: Parameters<typeof applyBeginnerGeneratedPlan>[4],
    expectedCandidateEdgeId: string,
  ) {
    const current = input.getCurrentSnapshot()
    const response = beginnerCandidates
    const plan = response?.generated_plans.find(
      (candidate) => (
        candidate.kind === kind
        && candidate.crease_pattern.edges[0]?.id === expectedCandidateEdgeId
      ),
    )
    if (
      !current
      || !response
      || !plan
      || !matchesBeginnerProjectBinding(response, current)
      || !input.confirm(input.copy.applyPlan)
    ) return
    const expectedProfile = current.beginner_design_profile
    void input.runNativeEdit((
      projectId,
      revision,
      projectInstanceId,
    ) => transport.applyPlan(
      projectId,
      revision,
      projectInstanceId,
      expectedProfile,
      kind,
      expectedCandidateEdgeId,
    ))
  }

  return {
    beginnerCandidates,
    beginnerCandidateBusy,
    consensusProgress,
    selectedConsensusPair,
    setSelectedConsensusPair,
    consensusSelectionDraft,
    beginnerSymmetricEstimate,
    beginnerSymmetricScale,
    setBeginnerSymmetricScale,
    beginnerSymmetricSpacing,
    setBeginnerSymmetricSpacing,
    requestBeginnerCandidates,
    cancelConsensusAnalysis,
    cancelBeginnerCandidates,
    excludeBeginnerConsensusAsset,
    toggleConsensusReference,
    saveConsensusReferences,
    requestBeginnerSymmetricEstimate,
    confirmBeginnerSymmetricEstimate,
    confirmAndAppendGenericTreeInstructions,
    confirmAndApplyBeginnerPlan,
  } as const
}
