const yearNode = document.getElementById("current-year");
const form = document.getElementById("waitlist-form");
const input = document.getElementById("waitlist-email");
const feedback = document.getElementById("waitlist-feedback");
const submitButton = form?.querySelector('button[type="submit"]');
const buttonLabel = submitButton?.querySelector(".button-label");
const storedEmailKey = "qanto-coming-soon-email";
const emailPattern = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

if (yearNode) {
  yearNode.textContent = String(new Date().getFullYear());
}

document.body.classList.add("is-ready");

const revealNodes = document.querySelectorAll("[data-reveal]");
if ("IntersectionObserver" in window) {
  const revealObserver = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          entry.target.classList.add("is-visible");
          entry.target.style.opacity = "1";
          entry.target.style.transform = "translateY(0)";
          revealObserver.unobserve(entry.target);
        }
      }
    },
    { threshold: 0.2 },
  );

  for (const node of revealNodes) {
    revealObserver.observe(node);
  }
} else {
  for (const node of revealNodes) {
    node.style.opacity = "1";
    node.style.transform = "translateY(0)";
  }
}

function setFeedback(message, state = "default") {
  if (!(feedback instanceof HTMLElement)) {
    return;
  }

  feedback.textContent = message;
  feedback.classList.remove("is-success", "is-error");

  if (state === "success") {
    feedback.classList.add("is-success");
  }

  if (state === "error") {
    feedback.classList.add("is-error");
  }
}

function setSubmitting(isSubmitting) {
  if (!(submitButton instanceof HTMLButtonElement)) {
    return;
  }

  submitButton.disabled = isSubmitting;

  if (buttonLabel instanceof HTMLElement) {
    buttonLabel.textContent = isSubmitting ? "Submitting..." : "Notify Me";
  }
}

async function submitWaitlist(email) {
  const response = await fetch("/api/subscribe", {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
    body: JSON.stringify({
      email,
      source: "coming-soon",
    }),
  });

  const payload = await response.json().catch(() => ({}));

  if (!response.ok) {
    throw new Error(payload?.message || "Unable to submit your request right now.");
  }

  return payload;
}

if (
  form instanceof HTMLFormElement &&
  input instanceof HTMLInputElement &&
  feedback instanceof HTMLElement
) {
  const storedEmail = localStorage.getItem(storedEmailKey);
  if (storedEmail) {
    input.value = storedEmail;
  }

  input.addEventListener("input", () => {
    if (feedback.textContent) {
      setFeedback("");
    }
  });

  form.addEventListener("submit", async (event) => {
    event.preventDefault();

    const email = input.value.trim().toLowerCase();

    if (!email) {
      setFeedback("Please enter your email address.", "error");
      input.focus();
      return;
    }

    if (!emailPattern.test(email)) {
      setFeedback("Please use a valid email address.", "error");
      input.focus();
      return;
    }

    setSubmitting(true);
    setFeedback("Submitting your request to the QANTO launch queue...");

    try {
      const payload = await submitWaitlist(email);
      localStorage.setItem(storedEmailKey, email);
      form.reset();
      input.value = email;
      setFeedback(payload.message || "You have been added to the waitlist.", "success");
    } catch (error) {
      setFeedback(error instanceof Error ? error.message : "Submission failed.", "error");
    } finally {
      setSubmitting(false);
    }
  });
}
