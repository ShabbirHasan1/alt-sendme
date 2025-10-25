import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import type { AlertDialogState, AlertType, TransferMetadata, TransferProgress } from '../types/sender'

export interface ImportProgress {
  processed: number
  total: number
  percentage: number
}

export interface UseSenderReturn {
  isSharing: boolean
  isImporting: boolean
  isTransporting: boolean
  isCompleted: boolean
  ticket: string | null
  selectedPath: string | null
  isLoading: boolean
  copySuccess: boolean
  alertDialog: AlertDialogState
  transferMetadata: TransferMetadata | null
  transferProgress: TransferProgress | null
  importProgress: ImportProgress | null
  
  handleFileSelect: (path: string) => void
  startSharing: () => Promise<void>
  stopSharing: () => Promise<void>
  copyTicket: () => Promise<void>
  showAlert: (title: string, description: string, type?: AlertType) => void
  closeAlert: () => void
  resetForNewTransfer: () => Promise<void>
}

export function useSender(): UseSenderReturn {
  const [isSharing, setIsSharing] = useState(false)
  const [isImporting, setIsImporting] = useState(false)
  const [isTransporting, setIsTransporting] = useState(false)
  const [isCompleted, setIsCompleted] = useState(false)
  const [ticket, setTicket] = useState<string | null>(null)
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [copySuccess, setCopySuccess] = useState(false)
  const [transferMetadata, setTransferMetadata] = useState<TransferMetadata | null>(null)
  const [transferProgress, setTransferProgress] = useState<TransferProgress | null>(null)
  const [importProgress, setImportProgress] = useState<ImportProgress | null>(null)
  const [transferStartTime, setTransferStartTime] = useState<number | null>(null)
  const [alertDialog, setAlertDialog] = useState<AlertDialogState>({
    isOpen: false,
    title: '',
    description: '',
    type: 'info'
  })

  useEffect(() => {
    let unlistenImportStart: UnlistenFn | undefined
    let unlistenImportCount: UnlistenFn | undefined
    let unlistenImportProgress: UnlistenFn | undefined
    let unlistenImportComplete: UnlistenFn | undefined
    let unlistenStart: UnlistenFn | undefined
    let unlistenProgress: UnlistenFn | undefined
    let unlistenComplete: UnlistenFn | undefined

    const setupListeners = async () => {
      unlistenImportStart = await listen('import-started', () => {
        console.log('📥 Import started');
        setIsImporting(true)
        setImportProgress(null)
      })

      unlistenImportCount = await listen('import-file-count', (event: any) => {
        const total = parseInt(event.payload as string, 10)
        console.log('📊 Import file count:', total)
        setImportProgress({ processed: 0, total, percentage: 0 })
      })

      unlistenImportProgress = await listen('import-progress', (event: any) => {
        try {
          const payload = event.payload as string
          const parts = payload.split(':')
          
          if (parts.length === 3) {
            const processed = parseInt(parts[0], 10)
            const total = parseInt(parts[1], 10)
            const percentage = parseInt(parts[2], 10)
            console.log(`📈 Import progress: ${processed}/${total} (${percentage}%)`)
            setImportProgress({ processed, total, percentage })
          }
        } catch (error) {
          console.error('❌ Failed to parse import progress event:', error)
        }
      })

      unlistenImportComplete = await listen('import-completed', () => {
        console.log('✅ Import completed')
        setIsImporting(false)
      })

      unlistenStart = await listen('transfer-started', () => {
        console.log('🚀 Transfer started')
        setIsTransporting(true)
        setIsCompleted(false)
        setTransferStartTime(Date.now())
        setTransferProgress(null)
        // Keep importProgress - don't reset it when transfer starts
      })

      unlistenProgress = await listen('transfer-progress', (event: any) => {
        try {
          const payload = event.payload as string
          const parts = payload.split(':')
          
          if (parts.length === 3) {
            const bytesTransferred = parseInt(parts[0], 10)
            const totalBytes = parseInt(parts[1], 10)
            const speedInt = parseInt(parts[2], 10)
            const speedBps = speedInt / 1000.0
            const percentage = totalBytes > 0 ? (bytesTransferred / totalBytes) * 100 : 0
            
            console.log(`📊 Transfer progress: ${(bytesTransferred / 1024 / 1024).toFixed(2)}MB/${(totalBytes / 1024 / 1024).toFixed(2)}MB (${percentage.toFixed(1)}%) @ ${(speedBps / 1024).toFixed(1)}KB/s`)
            
            setTransferProgress({
              bytesTransferred,
              totalBytes,
              speedBps,
              percentage
            })
          }
        } catch (error) {
          console.error('❌ Failed to parse progress event:', error)
        }
      })

      unlistenComplete = await listen('transfer-completed', async () => {
        console.log('✅ Transfer completed')
        setIsTransporting(false)
        setIsCompleted(true)
        setTransferProgress(null)
        
        const endTime = Date.now()
        const duration = transferStartTime ? endTime - transferStartTime : 0
        console.log(`⏱️  Transfer duration: ${(duration / 1000).toFixed(1)}s`)
        
        if (selectedPath) {
          try {
            console.log('📏 Getting file size for metadata...')
            const fileSize = await invoke<number>('get_file_size', { path: selectedPath })
            const fileName = selectedPath.split('/').pop() || 'Unknown'
            console.log(`📄 File: ${fileName}, Size: ${(fileSize / 1024 / 1024).toFixed(2)}MB`)
            const metadata = { 
              fileName, 
              fileSize, 
              duration, 
              startTime: transferStartTime || endTime, 
              endTime 
            }
            setTransferMetadata(metadata)
          } catch (error) {
            console.error('❌ Failed to get file size:', error)
            const fileName = selectedPath.split('/').pop() || 'Unknown'
            const metadata = { 
              fileName, 
              fileSize: 0, 
              duration, 
              startTime: transferStartTime || endTime, 
              endTime 
            }
            setTransferMetadata(metadata)
          }
        }
      })
    }

    setupListeners().catch((error) => {
      console.error('❌ Failed to set up event listeners:', error)
    })

    return () => {
      if (unlistenImportStart) unlistenImportStart()
      if (unlistenImportCount) unlistenImportCount()
      if (unlistenImportProgress) unlistenImportProgress()
      if (unlistenImportComplete) unlistenImportComplete()
      if (unlistenStart) unlistenStart()
      if (unlistenProgress) unlistenProgress()
      if (unlistenComplete) unlistenComplete()
    }
  }, [transferStartTime, selectedPath])

  const showAlert = (title: string, description: string, type: AlertType = 'info') => {
    setAlertDialog({ isOpen: true, title, description, type })
  }

  const closeAlert = () => {
    setAlertDialog(prev => ({ ...prev, isOpen: false }))
  }

  const handleFileSelect = (path: string) => {
    console.log('📁 File selected:', path);
    setSelectedPath(path)
  }

  const startSharing = async () => {
    if (!selectedPath) {
      console.warn('⚠️  No file selected for sharing');
      return
    }
    
    console.log('🚀 Starting file sharing for:', selectedPath);
    
    try {
      setIsLoading(true)
      console.log('📡 Invoking start_sharing command...');
      const result = await invoke<string>('start_sharing', { path: selectedPath })
      console.log('✅ Share started successfully, ticket received');
      setTicket(result)
      setIsSharing(true)
    } catch (error) {
      console.error('❌ Failed to start sharing:', error)
      showAlert('Sharing Failed', `Failed to start sharing: ${error}`, 'error')
    } finally {
      setIsLoading(false)
    }
  }

  const stopSharing = async () => {
    console.log('🛑 Stopping file sharing...');
    
    try {
      console.log('📡 Invoking stop_sharing command...');
      await invoke('stop_sharing')
      console.log('✅ Share stopped successfully');
      setIsSharing(false)
      setIsImporting(false)
      setIsTransporting(false)
      setIsCompleted(false)
      setTicket(null)
      setSelectedPath(null)
      setTransferMetadata(null)
      setTransferProgress(null)
      setImportProgress(null)
      setTransferStartTime(null)
    } catch (error) {
      console.error('❌ Failed to stop sharing:', error)
      showAlert('Stop Sharing Failed', `Failed to stop sharing: ${error}`, 'error')
    }
  }

  const resetForNewTransfer = async () => {
    await stopSharing()
  }

  const copyTicket = async () => {
    if (ticket) {
      console.log('📋 Copying ticket to clipboard...');
      try {
        await navigator.clipboard.writeText(ticket)
        console.log('✅ Ticket copied successfully');
        setCopySuccess(true)
        setTimeout(() => setCopySuccess(false), 2000)
      } catch (error) {
        console.error('❌ Failed to copy ticket:', error)
        showAlert('Copy Failed', `Failed to copy ticket: ${error}`, 'error')
      }
    } else {
      console.warn('⚠️  No ticket available to copy');
    }
  }

  return {
    isSharing,
    isImporting,
    isTransporting,
    isCompleted,
    ticket,
    selectedPath,
    isLoading,
    copySuccess,
    alertDialog,
    transferMetadata,
    transferProgress,
    importProgress,
    
    handleFileSelect,
    startSharing,
    stopSharing,
    copyTicket,
    showAlert,
    closeAlert,
    resetForNewTransfer
  }
}
