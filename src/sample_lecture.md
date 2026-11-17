# Gradient descent

_Built-in sample: a short excerpt of an English lecture with the usual speech-recognition errors._

**[00:00:00]** Okay so let's start. Last time we defined the loss, remember, the loss function L of theta tells us how wrong the model is on the training data.

**[00:00:12]** Today the question is how do we actually make it smaller. And the answer that basically all of deep learning uses is gradient decent.

**[00:00:24]** The idea is very simple. Imagine you are on a mountain in the fog, you can't see the valley, but you can feel the slope under your feet. So you take a step downhill.

**[00:00:38]** The gradient, nabla of L with respect to theta, points uphill, in the direction where the loss grows the fastest. So we go in the opposite direction.

**[00:00:51]** So the update rule is theta gets theta minus eta times the gradient of L. Write this down, this is the most important formula of the course, I'm not joking.

**[00:01:05]** Eta is the learning rate, it's how big the step is. And choosing it is, uh, more art than science honestly.

**[00:01:15]** If eta is too small you will converge but it takes forever. If it's too big you overshoot the minimum, you jump to the other side of the valley and the loss can even go up, it diverges.

**[00:01:31]** Let me draw it. Here's a parabola, the loss as a function of one parameter. With a small step you go down slowly like this, with a huge step you bounce from one side to the other.

**[00:01:47]** When I was a PhD student I spent two weeks debugging a network that wasn't learning and the problem was the learning rate was ten times too big. Two weeks. So check that first.

**[00:02:02]** Now, computing the gradient on the whole data set at every step is expensive. If you have a million examples you look at all of them just to take one step.

**[00:02:14]** So in practice we use stochastic gradient descent, SGD, where at each step we take a small random subset, a mini batch, say 32 or 64 examples, and estimate the gradient on that.

**[00:02:30]** The estimate is noisy but it's much cheaper, and the noise actually helps a bit to escape bad regions. We'll see that next week.

**[00:02:42]** One practical thing: for the exam you need to be able to write the update rule and explain what happens with a learning rate that is too large. That question comes up every year.

**[00:02:55]** Okay, questions? No? Then let's look at the code in the notebook, uh, the lab is on the course page.

