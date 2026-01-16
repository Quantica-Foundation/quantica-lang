// stdlib/ai.qc
/*
   Quantica Standard AI/ML Library
   Provides neural networks, training algorithms, and ML utilities
*/

import "stdlib/math.qc" as math

/* ============================================
   ACTIVATION FUNCTIONS
   ============================================ */

/* Sigmoid activation function */
func sigmoid(x: Float) -> Float:
    return 1.0 / (1.0 + math.exp(-x))

/* Sigmoid derivative */
func sigmoid_derivative(x: Float) -> Float:
    let s = sigmoid(x)
    return s * (1.0 - s)

/* Hyperbolic tangent activation */
func tanh(x: Float) -> Float:
    return math.tanh(x)

/* Tanh derivative */
func tanh_derivative(x: Float) -> Float:
    let t = math.tanh(x)
    return 1.0 - t * t

/* ReLU (Rectified Linear Unit) */
func relu(x: Float) -> Float:
    return math.relu(x)

/* ReLU derivative */
func relu_derivative(x: Float) -> Float:
    if x > 0.0:
        return 1.0
    return 0.0

/* Leaky ReLU */
func leaky_relu(x: Float, alpha: Float) -> Float:
    return math.leaky_relu(x, alpha)

/* Leaky ReLU derivative */
func leaky_relu_derivative(x: Float, alpha: Float) -> Float:
    if x > 0.0:
        return 1.0
    return alpha

/* Softmax for output layer */
func softmax(inputs: Float[]) -> Float[]:
    let n = len(inputs)
    mut max_val = inputs[0]

    // Find max for numerical stability
    mut i = 1
    while i < n:
        if inputs[i] > max_val:
            max_val = inputs[i]
        i = i + 1

    // Compute exp(x - max) and sum
    mut exp_sum = 0.0
    mut exp_values = []
    i = 0
    while i < n:
        let exp_val = math.exp(inputs[i] - max_val)
        exp_values = exp_values + [exp_val]
        exp_sum = exp_sum + exp_val
        i = i + 1

    // Normalize
    mut result = []
    i = 0
    while i < n:
        result = result + [exp_values[i] / exp_sum]
        i = i + 1

    return result

/* ============================================
   LOSS FUNCTIONS
   ============================================ */

/* Mean Squared Error loss */
func mse_loss(predicted: Float[], actual: Float[]) -> Float:
    let n = len(predicted)
    mut sum = 0.0
    mut i = 0

    while i < n:
        let diff = predicted[i] - actual[i]
        sum = sum + diff * diff
        i = i + 1

    return sum / to_float(n)

/* Mean Squared Error derivative */
func mse_loss_derivative(predicted: Float[], actual: Float[]) -> Float[]:
    let n = len(predicted)
    mut result = []
    mut i = 0

    while i < n:
        result = result + [2.0 * (predicted[i] - actual[i]) / to_float(n)]
        i = i + 1

    return result

/* Cross-Entropy loss */
func cross_entropy_loss(predicted: Float[], actual: Float[]) -> Float:
    let n = len(predicted)
    mut sum = 0.0
    mut i = 0

    while i < n:
        let p = math.max(predicted[i], 1e-15)  // Prevent log(0)
        sum = sum - actual[i] * math.ln(p)
        i = i + 1

    return sum

/* Binary Cross-Entropy loss */
func binary_cross_entropy(predicted: Float, actual: Float) -> Float:
    let p = math.clamp(predicted, 1e-15, 1.0 - 1e-15)
    return -(actual * math.ln(p) + (1.0 - actual) * math.ln(1.0 - p))

/* ============================================
   NEURAL NETWORK LAYER
   ============================================ */

Class Layer:
    public mut weights: Float[][]
    public mut biases: Float[]
    public mut input_size: Int
    public mut output_size: Int
    public mut activation: String

    private mut last_input: Float[]
    private mut last_output: Float[]
    private mut last_z: Float[]

    func init(in_size: Int, out_size: Int, activation_func: String):
        input_size = in_size
        output_size = out_size
        activation = activation_func

        weights = []
        let scale = math.sqrt(2.0 / to_float(in_size + out_size))

        mut i = 0
        while i < out_size:
            mut row = []
            mut j = 0
            while j < in_size:
                let seed = i * in_size + j
                let rand_val = (to_float(seed % 100) / 100.0 - 0.5) * 2.0 * scale
                row = row + [rand_val]
                j = j + 1
            weights = weights + [row]
            i = i + 1

        biases = []
        i = 0
        while i < out_size:
            biases = biases + [0.0]
            i = i + 1

        last_input = []
        last_output = []
        last_z = []

    func forward(input: Float[]) -> Float[]:
        self.last_input = input
        mut z = []

        mut i = 0
        while i < output_size:
            let sum = biases[i] + (weights[i] *** input)
            z = z + [sum]
            i = i + 1
        // ---------------------------

        self.last_z = z

        mut output = []
        i = 0
        while i < output_size:
            let activated = self.apply_activation(z[i])
            output = output + [activated]
            i = i + 1

        self.last_output = output
        return output

    func apply_activation(x: Float) -> Float:
        if activation == "sigmoid":
            return sigmoid(x)
        elif activation == "tanh":
            return tanh(x)
        elif activation == "relu":
            return relu(x)
        elif activation == "linear":
            return x
        return x

    func activation_derivative(x: Float) -> Float:
        if activation == "sigmoid":
            return sigmoid_derivative(x)
        elif activation == "tanh":
            return tanh_derivative(x)
        elif activation == "relu":
            return relu_derivative(x)
        elif activation == "linear":
            return 1.0
        return 1.0

    func backward(output_gradient: Float[], learning_rate: Float) -> Float[]:
        mut delta = []
        mut i = 0
        while i < output_size:
            let d = output_gradient[i] * self.activation_derivative(last_z[i])
            delta = delta + [d]
            i = i + 1


        /*
        i = 0
        while i < output_size:
            mut j = 0
            while j < input_size:
                let grad = delta[i] * last_input[j]
                weights[i][j] = weights[i][j] - learning_rate * grad
                j = j + 1

            // Update bias
            biases[i] = biases[i] - learning_rate * delta[i]
            i = i + 1
        */

        matrix_update(weights, delta, last_input, learning_rate, output_size, input_size)

        i = 0
        while i < output_size:
            biases[i] = biases[i] - learning_rate * delta[i]
            i = i + 1

        let input_gradient = compute_input_gradient(weights, delta, output_size, input_size)

        return input_gradient

/* ============================================
   NEURAL NETWORK
   ============================================ */

Class NeuralNetwork:
    private mut layers: Layer[]
    private mut num_layers: Int
    public mut learning_rate: Float

    func init(lr: Float):
        layers = []
        num_layers = 0
        learning_rate = lr

    func add_layer(in_size: Int, out_size: Int, activation: String):
        let layer = Layer(in_size, out_size, activation)
        layers = layers + [layer]
        num_layers = num_layers + 1

    func forward(input: Float[]) -> Float[]:
        //print("DEBUG forward: num_layers=" + to_string(num_layers) + ", layers.length=" + to_string(len(layers)))
        mut output = input
        mut i = 0
        while i < self.num_layers:
            output = self.layers[i].forward(output)
            i = i + 1
        return output

    func backward(loss_gradient: Float[]):
        mut gradient = loss_gradient
        mut i = self.num_layers - 1

        while i >= 0:
            gradient = self.layers[i].backward(gradient, learning_rate)
            i = i - 1

    func train_step(input: Float[], target: Float[]) -> Float:
        // Forward pass
        let predicted = self.forward(input)

        // Compute loss
        let loss = mse_loss(predicted, target)

        // Backward pass
        let loss_grad = mse_loss_derivative(predicted, target)
        self.backward(loss_grad)

        return loss

    func predict(input: Float[]) -> Float[]:
        return self.forward(input)

    func save(filename: String):
        print("Saving model to " + filename + "...")

        // Clear file
        file_write(filename, "")

        // Write Header
        file_append(filename, "lr:" + to_string(learning_rate) + "\n")
        file_append(filename, "layers:" + to_string(num_layers) + "\n")

        mut i = 0
        while i < self.num_layers:
            let layer = self.layers[i]

            // Write Layer Info
            mut header = "L" + to_string(i) + ":"
            header = header + to_string(layer.input_size) + ","
            header = header + to_string(layer.output_size) + ","
            header = header + layer.activation + "\n"
            file_append(filename, header)

            // Write Weights (Value by Value to prevent memory leak)
            mut r = 0
            while r < layer.output_size:
                mut c = 0
                while c < layer.input_size:
                    // ✅ OPTIMIZATION: Write directly to file instead of building a huge string
                    let val = to_string(layer.weights[r][c])

                    if c < layer.input_size - 1:
                        file_append(filename, val + ",")
                    else:
                        file_append(filename, val) // No comma at end of row

                    c = c + 1

                if r < layer.output_size - 1:
                    file_append(filename, ",") // Comma between rows

                r = r + 1

            file_append(filename, "\n")

            // Write Biases (Value by Value)
            let b_len = len(layer.biases)
            mut k = 0
            while k < b_len:
                let val = to_string(layer.biases[k])

                if k < b_len - 1:
                    file_append(filename, val + ",")
                else:
                    file_append(filename, val)

                k = k + 1

            // Newline after biases only if there are more layers or to end the block
            if i < num_layers - 1:
                file_append(filename, "\n")

            i = i + 1

        print("Model saved successfully.")

    func load(filename: String):
        print("Loading model from " + filename + "...")
        let content = file_read(filename)
        let lines = split(content, "\n")

        let lr_line = split(lines[0], ":")
        self.learning_rate = to_float(lr_line[1])

        let count_line = split(lines[1], ":")
        let total_layers = to_int(count_line[1])

        self.layers=[]
        self.num_layers = 0
        mut line_idx = 2
        mut i = 0
        while i < total_layers:
            let config_line_parts = split(lines[line_idx], ":")
            let config_str = config_line_parts[1]
            let config_parts = split(config_str, ",")
            let in_size = to_int(config_parts[0])
            let out_size = to_int(config_parts[1])
            let act = config_parts[2]

            self.add_layer(in_size, out_size, act)

            let w_vals = split(lines[line_idx + 1], ",")
            mut w_idx = 0
            mut r = 0
            while r < out_size:
                mut c = 0
                while c < in_size:
                    self.layers[i].weights[r][c] = to_float(w_vals[w_idx])
                    w_idx = w_idx + 1
                    c = c + 1
                r = r + 1
            let b_vals = split(lines[line_idx + 2], ",")
            mut k = 0
            while k < out_size:
                self.layers[i].biases[k] = to_float(b_vals[k])
                k = k + 1

            line_idx = line_idx + 3
            i = i + 1

        print("Model loaded.")

/* ============================================
   DATASET UTILITIES
   ============================================ */
Class Dataset:
    public mut inputs: Float[][]
    public mut targets: Float[][]
    public mut size: Int

    func init():
        inputs = []
        targets = []
        size = 0

    func add_sample(input: Float[], target: Float[]):
        inputs = inputs + [input]
        targets = targets + [target]
        size = size + 1

    func get_sample(index: Int) -> Dict:
        return {
            "input": inputs[index],
            "target": targets[index]
        }

    func shuffle():
        // Simple shuffle algorithm (Fisher-Yates)
        mut i = size - 1
        while i > 0:
            // In real implementation, use proper random number
            let j = i % (i + 1)

            // Swap inputs
            let temp_input = inputs[i]
            inputs[i] = inputs[j]
            inputs[j] = temp_input

            // Swap targets
            let temp_target = targets[i]
            targets[i] = targets[j]
            targets[j] = temp_target

            i = i - 1

/* ============================================
   TRAINING UTILITIES
   ============================================ */

/* Train a neural network */
func train_network(
    network: NeuralNetwork,
    dataset: Dataset,
    epochs: Int,
    batch_size: Int,
    verbose: Bool
) -> Float[]:
    mut losses = []

    mut epoch = 0
    while epoch < epochs:
        dataset.shuffle()

        mut epoch_loss = 0.0
        mut i = 0

        while i < dataset.size:
            let sample = dataset.get_sample(i)
            let loss = network.train_step(sample["input"], sample["target"])
            epoch_loss = epoch_loss + loss
            i = i + 1

        epoch_loss = epoch_loss / to_float(dataset.size)
        losses = losses + [epoch_loss]

        if verbose:
            print("Epoch " + to_string(epoch + 1) + "/" + to_string(epochs) +
                  " - Loss: " + to_string(epoch_loss))

        epoch = epoch + 1

    return losses

/* Evaluate network accuracy */
func evaluate_network(
    network: NeuralNetwork,
    dataset: Dataset,
    threshold: Float
) -> Float:
    if dataset.size == 0:
        print("Warning: Dataset is empty. Cannot evaluate accuracy.")
        return 0.0

    mut correct = 0
    mut i = 0

    while i < dataset.size:
        let sample = dataset.get_sample(i)
        let predicted = network.predict(sample["input"])
        let target = sample["target"]

        // Check if prediction is correct (for classification)
        let pred_class = argmax(predicted)
        let true_class = argmax(target)

        if pred_class == true_class:
            correct = correct + 1

        i = i + 1

    return to_float(correct) / to_float(dataset.size)

/* Find index of maximum value */
func argmax(values: Float[]) -> Int:
    mut max_idx = 0
    mut max_val = values[0]
    mut i = 1

    while i < len(values):
        if values[i] > max_val:
            max_val = values[i]
            max_idx = i
        i = i + 1

    return max_idx

/* ============================================
   COMMON ARCHITECTURES
   ============================================ */

/* Create a simple feedforward network */
func create_feedforward_network(
    input_size: Int,
    hidden_sizes: Int[],
    output_size: Int,
    learning_rate: Float
) -> NeuralNetwork:
    let network = NeuralNetwork(learning_rate)

    // Input to first hidden layer
    network.add_layer(input_size, hidden_sizes[0], "relu")

    // Hidden layers
    mut i = 1
    while i < len(hidden_sizes):
        network.add_layer(hidden_sizes[i - 1], hidden_sizes[i], "relu")
        i = i + 1

    // Hidden to output layer
    let last_hidden = hidden_sizes[len(hidden_sizes) - 1]
    network.add_layer(last_hidden, output_size, "sigmoid")

    return network

/* ============================================
   K-MEANS CLUSTERING
   ============================================ */

Class KMeans:
    private mut k: Int
    private mut centroids: Float[][]
    private mut max_iterations: Int

    func init(num_clusters: Int, max_iter: Int):
        k = num_clusters
        max_iterations = max_iter
        centroids = []

    func fit(data: Float[][], features: Int):
        // Initialize centroids (use first k data points)
        centroids = []
        mut i = 0
        while i < k:
            centroids = centroids + [data[i]]
            i = i + 1

        // Iterate until convergence or max iterations
        mut iteration = 0
        while iteration < max_iterations:
            // Assign points to clusters
            let assignments = self.assign_clusters(data, features)

            // Update centroids
            let new_centroids = self.update_centroids(data, assignments, features)

            // Check for convergence
            if self.centroids_equal(centroids, new_centroids, features):
                centroids = new_centroids
                iteration = max_iterations  // Break
            else:
                centroids = new_centroids

            iteration = iteration + 1

    func assign_clusters(data: Float[][], features: Int) -> Int[]:
        let n = len(data)
        mut assignments = []

        mut i = 0
        while i < n:
            mut min_dist = self.euclidean_distance(data[i], centroids[0], features)
            mut best_cluster = 0

            mut j = 1
            while j < k:
                let dist = self.euclidean_distance(data[i], centroids[j], features)
                if dist < min_dist:
                    min_dist = dist
                    best_cluster = j
                j = j + 1

            assignments = assignments + [best_cluster]
            i = i + 1

        return assignments

    func update_centroids(data: Float[][], assignments: Int[], features: Int) -> Float[][]:
        mut new_centroids = []

        mut cluster = 0
        while cluster < k:
            mut centroid = []
            mut counts = []

            // Initialize to zeros
            mut f = 0
            while f < features:
                centroid = centroid + [0.0]
                counts = counts + [0]
                f = f + 1

            // Sum all points in this cluster
            mut i = 0
            while i < len(data):
                if assignments[i] == cluster:
                    f = 0
                    while f < features:
                        centroid[f] = centroid[f] + data[i][f]
                        counts[f] = counts[f] + 1
                        f = f + 1
                i = i + 1

            // Average
            f = 0
            while f < features:
                if counts[f] > 0:
                    centroid[f] = centroid[f] / to_float(counts[f])
                f = f + 1

            new_centroids = new_centroids + [centroid]
            cluster = cluster + 1

        return new_centroids

    func euclidean_distance(point1: Float[], point2: Float[], features: Int) -> Float:
        mut sum = 0.0
        mut i = 0
        while i < features:
            let diff = point1[i] - point2[i]
            sum = sum + diff * diff
            i = i + 1
        return math.sqrt(sum)

    func centroids_equal(c1: Float[][], c2: Float[][], features: Int) -> Bool:
        mut i = 0
        while i < k:
            mut j = 0
            while j < features:
                if math.abs(c1[i][j] - c2[i][j]) > 1e-6:
                    return False
                j = j + 1
            i = i + 1
        return True

    func predict(point: Float[], features: Int) -> Int:
        mut min_dist = self.euclidean_distance(point, centroids[0], features)
        mut best_cluster = 0

        mut i = 1
        while i < k:
            let dist = self.euclidean_distance(point, centroids[i], features)
            if dist < min_dist:
                min_dist = dist
                best_cluster = i
            i = i + 1

        return best_cluster

/* ============================================
   LINEAR REGRESSION
   ============================================ */

Class LinearRegression:
    private mut weights: Float[]
    private mut bias: Float
    private mut learning_rate: Float

    func init(features: Int, lr: Float):
        learning_rate = lr
        bias = 0.0

        // Initialize weights
        weights = []
        mut i = 0
        while i < features:
            weights = weights + [0.0]
            i = i + 1

    func predict(x: Float[]) -> Float:
        mut y = bias
        mut i = 0
        while i < len(weights):
            y = y + weights[i] * x[i]
            i = i + 1
        return y

    func train_step(x: Float[], y_true: Float):
        let y_pred = self.predict(x)
        let error = y_pred - y_true

        // Update weights
        mut i = 0
        while i < len(weights):
            weights[i] = weights[i] - learning_rate * error * x[i]
            i = i + 1

        // Update bias
        bias = bias - learning_rate * error

    func fit(X: Float[][], y: Float[], epochs: Int):
        let n = len(X)

        mut epoch = 0
        while epoch < epochs:
            mut i = 0
            while i < n:
                self.train_step(X[i], y[i])
                i = i + 1
            epoch = epoch + 1

/* ============================================
   GRADIENT DESCENT OPTIMIZER
   ============================================ */

Class Optimizer:
    private mut learning_rate: Float
    private mut momentum: Float
    private mut velocity: Float[]

    func init(lr: Float, mom: Float):
        learning_rate = lr
        momentum = mom
        velocity = []

    func update(weights: Float[], gradients: Float[]) -> Float[]:
        let n = len(weights)

        // Initialize velocity if needed
        if len(velocity) == 0:
            mut i = 0
            while i < n:
                velocity = velocity + [0.0]
                i = i + 1

        // Update with momentum
        mut new_weights = []
        mut i = 0
        while i < n:
            velocity[i] = momentum * velocity[i] - learning_rate * gradients[i]
            new_weights = new_weights + [weights[i] + velocity[i]]
            i = i + 1

        return new_weights

/* ============================================
   FEATURE ENGINEERING
   ============================================ */

/* Normalize features to [0, 1] */
func normalize_features(data: Float[][], features: Int) -> Float[][]:
    let n = len(data)

    // Find min and max for each feature
    mut mins = []
    mut maxs = []
    mut i = 0
    while i < features:
        mins = mins + [data[0][i]]
        maxs = maxs + [data[0][i]]
        i = i + 1

    mut row = 1
    while row < n:
        mut col = 0
        while col < features:
            if data[row][col] < mins[col]:
                mins[col] = data[row][col]
            if data[row][col] > maxs[col]:
                maxs[col] = data[row][col]
            col = col + 1
        row = row + 1

    // Normalize
    mut normalized = []
    row = 0
    while row < n:
        mut norm_row = []
        mut col = 0
        while col < features:
            let range = maxs[col] - mins[col]
            if range > 1e-10:
                let norm_val = (data[row][col] - mins[col]) / range
                norm_row = norm_row + [norm_val]
            else:
                norm_row = norm_row + [0.0]
            col = col + 1
        normalized = normalized + [norm_row]
        row = row + 1

    return normalized

/* Standardize features (zero mean, unit variance) */
func standardize_features(data: Float[][], features: Int) -> Float[][]:
    let n = len(data)

    // Compute mean for each feature
    mut means = []
    mut i = 0
    while i < features:
        means = means + [0.0]
        i = i + 1

    mut row = 0
    while row < n:
        mut col = 0
        while col < features:
            means[col] = means[col] + data[row][col]
            col = col + 1
        row = row + 1

    mut col = 0
    while col < features:
        means[col] = means[col] / to_float(n)
        col = col + 1

    // Compute standard deviation
    mut stds = []
    col = 0
    while col < features:
        stds = stds + [0.0]
        col = col + 1

    row = 0
    while row < n:
        col = 0
        while col < features:
            let diff = data[row][col] - means[col]
            stds[col] = stds[col] + diff * diff
            col = col + 1
        row = row + 1

    col = 0
    while col < features:
        stds[col] = math.sqrt(stds[col] / to_float(n))
        col = col + 1

    // Standardize
    mut standardized = []
    row = 0
    while row < n:
        mut std_row = []
        col = 0
        while col < features:
            if stds[col] > 1e-10:
                let std_val = (data[row][col] - means[col]) / stds[col]
                std_row = std_row + [std_val]
            else:
                std_row = std_row + [0.0]
            col = col + 1
        standardized = standardized + [std_row]
        row = row + 1

    return standardized

/* ============================================
   EXAMPLE USAGE
   ============================================ */

/* Example: Train a simple XOR network */
/*
func example_xor_network():
    print("Training XOR Neural Network...")

    // Create network: 2 inputs -> 4 hidden -> 1 output
    let network = NeuralNetwork(0.5)
    network.add_layer(2, 4, "sigmoid")
    network.add_layer(4, 1, "sigmoid")

    // Create dataset
    let dataset = Dataset()
    dataset.add_sample([0.0, 0.0], [0.0])
    dataset.add_sample([0.0, 1.0], [1.0])
    dataset.add_sample([1.0, 0.0], [1.0])
    dataset.add_sample([1.0, 1.0], [0.0])

    // Train
    let losses = train_network(network, dataset, 1000, 1, False)

    // Test
    print("Testing XOR network:")
    print("0 XOR 0 = " + to_string(network.predict([0.0, 0.0])[0]))
    print("0 XOR 1 = " + to_string(network.predict([0.0, 1.0])[0]))
    print("1 XOR 0 = " + to_string(network.predict([1.0, 0.0])[0]))
    print("1 XOR 1 = " + to_string(network.predict([1.0, 1.0])[0]))



example_xor_network()
*/