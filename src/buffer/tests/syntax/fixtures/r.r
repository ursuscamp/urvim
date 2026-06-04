# R syntax fixture
# Multi-line comment
# is just multiple hash lines

value <- "hello"
count <- 42
flag <- TRUE
false_flag <- FALSE
null_val <- NULL
inf_val <- Inf
nan_val <- NaN
na_val <- NA
pi_val <- pi
char_val <- 'x'

if (flag) {
  print(value)
} else {
  print("not flag")
}

for (i in 1:10) {
  print(paste("count:", i))
}

while (count > 0) {
  count <- count - 1
}

greet <- function(name = "world") {
  result <- paste("Hello,", name)
  return(result)
}

result <- greet("Ada")

my_list <- list(
  name = "Ada",
  age = 42,
  scores = c(1, 2, 3)
)

my_matrix <- matrix(1:9, nrow = 3, ncol = 3)

my_factor <- factor(c("low", "medium", "high"))

library(dplyr)
require(ggplot2)

my_summary <- my_list %>%
  paste(collapse = ",") %>%
  nchar()

raw_string <- R"(hello
world)"
