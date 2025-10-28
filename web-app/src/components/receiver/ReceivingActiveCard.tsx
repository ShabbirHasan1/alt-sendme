import type { TransferProgress, ExportProgress } from '../../types/sender'
import { TransferProgressBar } from '../sender/TransferProgressBar'
import { formatBytes } from '../../lib/utils'

interface ReceivingActiveCardProps {
  isReceiving: boolean
  isTransporting: boolean
  isExporting?: boolean
  isCompleted: boolean
  ticket: string
  transferProgress: TransferProgress | null
  exportProgress?: ExportProgress | null
  resumedFrom?: number | null
  fileNames: string[]
  onReceive: () => Promise<void>
  onStopReceiving: () => Promise<void>
}

export function ReceivingActiveCard({ 
  isTransporting,
  isExporting,
  isCompleted,
  transferProgress,
  exportProgress,
  resumedFrom,
  onStopReceiving: _onStopReceiving 
}: ReceivingActiveCardProps) {
  // Determine the current state and colors
  const getStatusColor = () => {
    if (isCompleted) return 'rgb(45, 120, 220)' // Blue - completed
    if (isExporting) return 'rgba(255, 193, 7, 0.8)' // Orange - saving
    if (isTransporting) return 'rgba(37, 211, 101, 0.687)' // Green - transporting
    return '#B7B7B7' // Gray - connecting
  }

  const getStatusText = () => {
    if (isCompleted) return 'Download completed'
    if (isExporting) return 'Saving files...'
    if (isTransporting) return 'Downloading in progress'
    return 'Connecting to sender'
  }


  const statusColor = getStatusColor()
  const statusText = getStatusText()

  return (
    <div className="space-y-4">
      <div className="p-4 rounded-lg absolute top-0 left-0">
       
        <div className="flex items-center mb-2">
          <div 
            className="h-2 w-2 rounded-full mr-2" 
            style={{ backgroundColor: statusColor }}
          ></div>
          <p 
            className="text-sm font-medium" 
            style={{ color: statusColor }}
          >
            {statusText}
          </p>
        </div>
      </div>
      
      <p className="text-xs text-center" style={{ color: 'rgba(255, 255, 255, 0.7)' }}>
        Keep this app open while downloading files
      </p>
      
      {/* Show resume notification if download resumed */}
      {resumedFrom && resumedFrom > 0 && (
        <div 
          className="p-3 rounded-md border"
          style={{
            backgroundColor: 'rgba(37, 211, 101, 0.1)',
            borderColor: 'rgba(37, 211, 101, 0.3)',
            color: 'rgba(37, 211, 101, 1)'
          }}
        >
          <p className="text-sm font-medium">
            Resuming download from {formatBytes(resumedFrom)}
          </p>
        </div>
      )}
        
      {/* Show export progress when saving files */}
      {isExporting && exportProgress && (
        <div className="space-y-3">
          <div className="space-y-2">
            <div className="flex items-center justify-between text-xs" style={{ color: 'rgba(255, 255, 255, 0.7)' }}>
              <span>Export Progress</span>
              <span>{exportProgress.percentage.toFixed(1)}%</span>
            </div>
            
            {/* Progress bars container */}
            <div className="flex gap-1 items-end h-8">
              {Array.from({ length: 30 }).map((_, index) => {
                const filledBars = Math.floor((exportProgress.percentage / 100) * 30)
                const isFilled = index < filledBars
                const isPartiallyFilled = index === filledBars && exportProgress.percentage % (100 / 30) > 0
                
                // Calculate partial fill height for smoother animation
                let fillPercentage = 100
                if (isPartiallyFilled) {
                  const barProgress = (exportProgress.percentage % (100 / 30)) / (100 / 30)
                  fillPercentage = barProgress * 100
                } else if (!isFilled) {
                  fillPercentage = 0
                }

                return (
                  <div
                    key={index}
                    className="relative flex-1 rounded-sm transition-all duration-300 ease-in-out"
                    style={{
                      backgroundColor: 'rgba(255, 255, 255, 0.1)',
                      minWidth: '3px',
                      height: '100%',
                    }}
                  >
                    {/* Filled portion */}
                    <div
                      className="absolute bottom-0 left-0 right-0 rounded-sm transition-all duration-300 ease-in-out"
                      style={{
                        backgroundColor: 'rgba(37, 211, 101, 0.687)',
                        height: `${fillPercentage}%`,
                      }}
                    />
                  </div>
                )
              })}
            </div>

            {/* File count display */}
            <div className="flex items-center justify-between text-xs" style={{ color: 'rgba(255, 255, 255, 0.6)' }}>
              <span>Copying files to destination folder...</span>
              <span>{exportProgress.current} / {exportProgress.total} files</span>
            </div>
          </div>
        </div>
      )}
        
      {/* Show progress bar when transporting */}
      {isTransporting && transferProgress && (
        <TransferProgressBar progress={transferProgress} />
      )}
       
      {/* <button
        onClick={onStopReceiving}
        className="absolute top-0 right-6 py-2 px-4 rounded-md font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-offset-2 flex items-center justify-center"
        style={{
          backgroundColor: 'var(--app-destructive)',
          color: 'var(--app-destructive-fg)',
        }}
      >
        <span className="text-xl"> ⏹</span>
      </button> */}
    </div>
  )
}