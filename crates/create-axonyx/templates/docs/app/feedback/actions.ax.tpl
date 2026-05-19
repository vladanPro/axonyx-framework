action SendFeedback
  input:
    name?: string = "anonymous"
    message: string
    tone?: string = "idea"

  patch feedbackStatus = input.tone
  revalidate "/feedback"
  return ok
