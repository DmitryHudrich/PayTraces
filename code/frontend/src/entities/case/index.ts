export {
  CASE_PRIORITIES,
  CASE_STATUSES,
  STATUS_LABEL,
  priorityClasses,
  statusClasses,
} from '@/entities/case/model/case'
export type {
  CaseAddress,
  CaseAssignment,
  CaseDetail,
  CaseNote,
  CasePriority,
  CaseStatus,
  CaseSummary,
} from '@/entities/case/model/case'
export {
  addCaseAddress,
  assignCase,
  closeCase,
  createCase,
  getCase,
  listCases,
} from '@/entities/case/api/cases'
export {
  caseKeys,
  useAddCaseAddressMutation,
  useAssignCaseMutation,
  useCaseQuery,
  useCasesQuery,
  useCloseCaseMutation,
  useCreateCaseMutation,
} from '@/entities/case/api/queries'
